use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    engine::{Engine, SimulationEngine},
    types::{PlayerAction, SimError, TankState},
};

/// Current schema version. Bump this when the save format changes.
///
/// When you bump from N to N+1, you **must** also append a migration function
/// to [`MIGRATIONS`]. See the migration contract below.
pub const SCHEMA_VERSION: u32 = 3;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Oldest schema version that the migration chain can handle.
const MIN_SUPPORTED_SCHEMA: u32 = 2;

// ---------------------------------------------------------------------------
// Migration registry
// ---------------------------------------------------------------------------

/// A migration function transforms a raw JSON `SaveFile` from version N to
/// N+1.  The `&mut Value` is the top-level object; access state fields via
/// `value["state"]["water"]["field_name"]` etc.
///
/// Operating on raw JSON means migrations can handle field renames, removals,
/// additions, and value transforms regardless of the current Rust struct shape.
type MigrationFn = fn(&mut Value);

/// Ordered list of migrations.  Index 0 migrates `MIN_SUPPORTED_SCHEMA` →
/// `MIN_SUPPORTED_SCHEMA + 1`, index 1 migrates `MIN_SUPPORTED_SCHEMA + 1` →
/// `MIN_SUPPORTED_SCHEMA + 2`, and so on.
///
/// **Invariant**: `MIGRATIONS.len() == (SCHEMA_VERSION - MIN_SUPPORTED_SCHEMA)`
///
/// # Migration contract — how to add a new migration
///
/// 1. Write a function `fn migrate_vN_to_vM(value: &mut Value)` that
///    transforms the top-level JSON from schema N to M (where M = N + 1).
///    Access state via `value["state"]`.
///
/// 2. Append that function to [`MIGRATIONS`].
///
/// 3. Bump [`SCHEMA_VERSION`] to M.
///
/// 4. Add a test that constructs a version-N JSON blob, calls
///    `SaveFile::from_json`, and asserts the migration applied correctly.
///
/// ## What migrations can do
///
/// Because they operate on `serde_json::Value`, migrations can:
///
/// - **Rename fields**: move `state["old"]` → `state["new"]`.
/// - **Add fields with defaults**: insert `state["new_field"] = json!(0.0)`.
/// - **Remove fields**: `state.as_object_mut().remove("old_field")`.
/// - **Transform values**: read, compute, write — arbitrary numeric/structural
///   transforms on the JSON tree.
///
/// For simple additions where serde's `#[serde(default)]` suffices, you may
/// rely on that instead of writing a migration, but you still **must** bump
/// `SCHEMA_VERSION` and add a no-op migration entry so the version chain
/// stays contiguous.
///
/// ## Ordering guarantees
///
/// Migrations run in strict version order (2→3→4→…→current). Each migration
/// may assume its input matches the previous version's schema exactly.
const MIGRATIONS: &[MigrationFn] = &[
    // Index 0: schema 2 → 3
    // Rescale dissolved-chemistry totals from gross-volume basis to net-water
    // volume (accounting for substrate displacement).
    migrate_v2_to_v3,
];

// Compile-time check: MIGRATIONS length must equal SCHEMA_VERSION - MIN_SUPPORTED_SCHEMA.
const _: () = assert!(
    MIGRATIONS.len() == (SCHEMA_VERSION - MIN_SUPPORTED_SCHEMA) as usize,
    "MIGRATIONS length must equal SCHEMA_VERSION - MIN_SUPPORTED_SCHEMA"
);

// ---------------------------------------------------------------------------
// SaveFile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveFile {
    pub schema_version: u32,
    pub app_version: String,
    pub state: TankState,
    pub queued_actions: Vec<PlayerAction>,
}

impl SaveFile {
    pub fn new(state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app_version: APP_VERSION.to_string(),
            state,
            queued_actions,
        }
    }

    pub fn from_engine(engine: &Engine) -> Self {
        Self::new(engine.full_state().clone(), engine.queued_actions())
    }

    pub fn to_json_pretty(&self) -> Result<String, SimError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| SimError::Serialization(error.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, SimError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|error| SimError::Deserialization(error.to_string()))?;
        Self::from_value(value)
    }

    /// Load a `SaveFile` from a pre-parsed JSON `Value`, applying any
    /// necessary schema migrations. This avoids a redundant
    /// `Value → String → Value` round-trip when the caller already has a
    /// `Value` (e.g. from an HTTP JSON body).
    pub fn from_value(mut value: Value) -> Result<Self, SimError> {
        let raw_version = value
            .get("schema_version")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        // Reject versions that overflow u32 — they are from an unknown future
        // format and must not silently wrap to a lower version.
        let file_version =
            u32::try_from(raw_version).map_err(|_| SimError::SchemaVersionTooNew {
                actual: u32::MAX,
                max_supported: SCHEMA_VERSION,
            })?;

        // Reject saves from the future.
        if file_version > SCHEMA_VERSION {
            return Err(SimError::SchemaVersionTooNew {
                actual: file_version,
                max_supported: SCHEMA_VERSION,
            });
        }

        // Reject saves too old for the migration chain.
        if file_version < MIN_SUPPORTED_SCHEMA {
            return Err(SimError::SchemaVersionTooOld {
                actual: file_version,
                min_supported: MIN_SUPPORTED_SCHEMA,
            });
        }

        // Apply migrations sequentially: file_version → file_version+1 → … → SCHEMA_VERSION
        for v in file_version..SCHEMA_VERSION {
            let idx = (v - MIN_SUPPORTED_SCHEMA) as usize;
            MIGRATIONS[idx](&mut value);
        }

        // Stamp the migrated version so deserialization sees the current schema.
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "schema_version".to_string(),
                Value::Number(SCHEMA_VERSION.into()),
            );
        }

        // Detect whether the stability tracker was serialized before we
        // deserialize (it may be absent in old saves that predate the field).
        let stability_tracker_present = has_serialized_stability_tracker(&value);

        let mut save: Self = serde_json::from_value(value)
            .map_err(|error| SimError::Deserialization(error.to_string()))?;

        reseed_stability_tracker_if_missing(&mut save.state, stability_tracker_present);

        Ok(save)
    }

    pub fn into_engine(self) -> Result<Engine, SimError> {
        // Validate state invariants before exposing the engine.
        let mut state = self.state;
        crate::invariants::enforce_invariants(&mut state)?;

        // Validate queued actions including state-aware checks (source
        // profile existence, shrimp removal counts) by routing through the
        // same path as live action submission.
        let mut engine = Engine::from_parts(state, vec![]);
        for action in self.queued_actions {
            engine.apply_action(action)?;
        }
        Ok(engine)
    }
}

// ---------------------------------------------------------------------------
// Migration implementations
// ---------------------------------------------------------------------------

/// Schema 2 → 3: dissolved-chemistry totals were stored on a gross-volume
/// basis.  Schema 3 uses net-water volume (after substrate displacement).
/// This migration rescales every dissolved total by `net / gross`.
fn migrate_v2_to_v3(value: &mut Value) {
    let state = match value.get_mut("state") {
        Some(s) => s,
        None => return,
    };

    // Compute gross and net volumes from geometry + substrate layers.
    let geometry = &state["geometry"];
    let length = geometry["length_cm"].as_f64().unwrap_or(0.0);
    let width = geometry["width_cm"].as_f64().unwrap_or(0.0);
    let fill_height = geometry["fill_height_cm"].as_f64().unwrap_or(0.0);
    let gross_volume_l = length * width * fill_height / 1000.0;

    let substrate_depth: f64 = state["substrate_layers"]
        .as_array()
        .map(|layers| {
            layers
                .iter()
                .filter_map(|l| l["depth_cm"].as_f64())
                .map(|d| d.max(0.0))
                .sum()
        })
        .unwrap_or(0.0);

    let capped_depth = substrate_depth.clamp(0.0, fill_height.max(0.0));
    let displacement_l = length * width * capped_depth / 1000.0;
    let net_volume_l = (gross_volume_l - displacement_l).max(0.0);

    let scale = if gross_volume_l > f64::EPSILON && gross_volume_l.is_finite() {
        (net_volume_l / gross_volume_l).max(0.0)
    } else {
        0.0
    };

    // Rescale every dissolved total field in the water object.
    let water = match state.get_mut("water") {
        Some(w) => w,
        None => return,
    };

    let dissolved_fields = [
        "ammonia_total_mg_n_total",
        "nitrite_mg_n_total",
        "nitrate_mg_n_total",
        "phosphate_mg_p_total",
        "dissolved_oxygen_mg_total",
        "dissolved_inorganic_carbon_mg_c_total",
        "dissolved_organic_carbon_mg_c_total",
        "dissolved_organic_nitrogen_mg_n_total",
        "alkalinity_meq_total",
        "calcium_mg_total",
        "magnesium_mg_total",
        "sodium_mg_total",
        "potassium_mg_total",
        "bicarbonate_mg_total",
        "chloride_mg_total",
        "sulfate_mg_total",
    ];

    for field in &dissolved_fields {
        if let Some(val) = water.get_mut(*field) {
            if let Some(num) = val.as_f64() {
                *val = serde_json::json!(num * scale);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_serialized_stability_tracker(value: &Value) -> bool {
    value
        .get("state")
        .and_then(|state| state.get("stability_tracker"))
        .is_some()
}

fn reseed_stability_tracker_if_missing(state: &mut TankState, stability_tracker_present: bool) {
    if !stability_tracker_present {
        state.reseed_stability_tracker();
    }
}
