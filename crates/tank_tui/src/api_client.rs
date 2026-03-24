use anyhow::{bail, Context};
use reqwest::blocking::Client;
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
        resp.json().context("failed to parse snapshot")
    }

    pub fn step_hours(&self, hours: u32) -> anyhow::Result<TankSnapshot> {
        let resp = self
            .client
            .post(format!("{}/time/step", self.base_url))
            .json(&StepRequest { hours })
            .send()
            .context("POST /time/step failed")?;
        check_status(&resp)?;
        resp.json().context("failed to parse step response")
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
        let body: LoadResponse = resp.json().context("failed to parse load response")?;
        Ok(body.snapshot)
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

#[derive(serde::Deserialize)]
struct LoadResponse {
    snapshot: TankSnapshot,
}

fn check_status(resp: &reqwest::blocking::Response) -> anyhow::Result<()> {
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    bail!("HTTP {status}")
}
