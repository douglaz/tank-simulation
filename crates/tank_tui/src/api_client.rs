use anyhow::{bail, Context};
use reqwest::blocking::Client;
use serde_json::Value;
use tank_core::{PlayerAction, SaveFile, TankSnapshot};

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

#[derive(serde::Serialize)]
struct StepRequest {
    hours: u32,
}

impl ApiClient {
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn get_snapshot(&self) -> anyhow::Result<TankSnapshot> {
        let resp = self
            .client
            .get(format!("{}/snapshot", self.base_url))
            .send()
            .context("GET /snapshot failed")?;
        check_status(&resp)?;
        let body: Value = resp.json().context("failed to parse snapshot JSON")?;
        parse_snapshot_value(body)
    }

    pub fn step_hours(&self, hours: u32) -> anyhow::Result<TankSnapshot> {
        let resp = self
            .client
            .post(format!("{}/time/step", self.base_url))
            .json(&StepRequest { hours })
            .send()
            .context("POST /time/step failed")?;
        check_status(&resp)?;
        let body: Value = resp.json().context("failed to parse step response JSON")?;
        parse_snapshot_value(body)
    }

    pub fn apply_action(&self, action: &PlayerAction) -> anyhow::Result<()> {
        let resp = self
            .client
            .post(format!("{}/actions", self.base_url))
            .json(action)
            .send()
            .context("POST /actions failed")?;
        check_status(&resp)?;
        Ok(())
    }

    pub fn get_save(&self) -> anyhow::Result<SaveFile> {
        let resp = self
            .client
            .get(format!("{}/save", self.base_url))
            .send()
            .context("GET /save failed")?;
        check_status(&resp)?;
        resp.json().context("failed to parse save file")
    }

    pub fn post_load(&self, save: &SaveFile) -> anyhow::Result<TankSnapshot> {
        let resp = self
            .client
            .post(format!("{}/load", self.base_url))
            .json(save)
            .send()
            .context("POST /load failed")?;
        check_status(&resp)?;
        let body: Value = resp.json().context("failed to parse load response JSON")?;
        parse_load_response(body)
    }

    pub fn get_source_water_ids(&self) -> anyhow::Result<Vec<String>> {
        let resp = self
            .client
            .get(format!("{}/source-water", self.base_url))
            .send()
            .context("GET /source-water failed")?;
        check_status(&resp)?;
        resp.json().context("failed to parse source water IDs")
    }
}

fn check_status(resp: &reqwest::blocking::Response) -> anyhow::Result<()> {
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    bail!("HTTP {status}")
}

fn parse_load_response(mut value: Value) -> anyhow::Result<TankSnapshot> {
    let snapshot = value
        .as_object_mut()
        .and_then(|object| object.remove("snapshot"))
        .context("load response missing snapshot")?;
    serde_json::from_value(snapshot).context("failed to deserialize load snapshot")
}

fn parse_snapshot_value(value: Value) -> anyhow::Result<TankSnapshot> {
    serde_json::from_value(value).context("failed to deserialize snapshot")
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map};
    use tank_core::{
        rng::SimSeed, TankState, ESTIMATED_TDS_OMITTED_CONTRIBUTORS,
        ESTIMATED_TDS_TRACKED_MAJOR_IONS, LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES,
    };

    use super::{parse_load_response, parse_snapshot_value};

    fn canonical_snapshot_value(seed: SimSeed) -> serde_json::Value {
        let snapshot = tank_core::TankSnapshot::from_state(&TankState::new(seed));
        serde_json::to_value(&snapshot).expect("snapshot should serialize")
    }

    fn legacy_snapshot_value(seed: SimSeed) -> serde_json::Value {
        let mut value = canonical_snapshot_value(seed);
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        for (legacy, canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
            let canonical_value = object
                .remove(canonical)
                .expect("compatibility test expects canonical chemistry field");
            object.insert(legacy.to_string(), canonical_value);
        }
        value
    }

    fn enriched_api_snapshot_value(seed: SimSeed) -> serde_json::Value {
        let mut value = canonical_snapshot_value(seed);
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        for (legacy, canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
            let canonical_value = object
                .get(canonical)
                .cloned()
                .expect("compatibility test expects canonical chemistry field");
            object.insert(legacy.to_string(), canonical_value);
        }
        object.insert(
            "chemistry_field_semantics".to_string(),
            json!({
                "tan_mg_n_per_l": "Total ammonia nitrogen, mg N/L.",
                "estimated_tds_7_ion_mg_per_l": "Estimated TDS from 7 tracked major ions only, in mg/L."
            }),
        );
        object.insert(
            "estimated_tds_scope".to_string(),
            json!({
                "tracked_major_ions": ESTIMATED_TDS_TRACKED_MAJOR_IONS,
                "omitted_contributors": ESTIMATED_TDS_OMITTED_CONTRIBUTORS,
            }),
        );
        object.insert(
            "legacy_chemistry_aliases".to_string(),
            serde_json::Value::Object(Map::from_iter(
                LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES
                    .into_iter()
                    .map(|(legacy, canonical)| (legacy.to_string(), json!(canonical))),
            )),
        );
        value
    }

    #[test]
    fn parses_legacy_snapshot_chemistry_field_names() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(777)));
        let value = legacy_snapshot_value(SimSeed(777));

        let parsed = parse_snapshot_value(value).expect("legacy snapshot should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_pre_stage_snapshot_payloads_without_stage_totals() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(778)));
        let mut value = canonical_snapshot_value(SimSeed(778));
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        object.remove("total_shrimp_count");
        object.remove("sub_adult_count");

        let parsed = parse_snapshot_value(value).expect("pre-stage snapshot should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn keeps_canonical_snapshot_fields_when_legacy_aliases_are_also_present() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(888)));
        let mut value = serde_json::to_value(&expected).expect("snapshot should serialize");
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        for (legacy, _canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
            object.insert(legacy.to_string(), json!(-999.0));
        }

        let parsed = parse_snapshot_value(value).expect("canonical snapshot should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_enriched_api_snapshot_response() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(889)));
        let value = enriched_api_snapshot_value(SimSeed(889));

        let parsed = parse_snapshot_value(value).expect("enriched API snapshot should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_load_response_with_legacy_snapshot_chemistry_field_names() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(999)));
        let body = json!({ "snapshot": legacy_snapshot_value(SimSeed(999)) });

        let parsed = parse_load_response(body).expect("legacy load response should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_load_response_with_enriched_api_snapshot() {
        let expected = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(1000)));
        let body = json!({
            "status": "loaded",
            "snapshot": enriched_api_snapshot_value(SimSeed(1000)),
        });

        let parsed = parse_load_response(body).expect("enriched load response should deserialize");

        assert_eq!(parsed, expected);
    }
}
