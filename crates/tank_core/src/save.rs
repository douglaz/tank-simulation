use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    engine::{Engine, SimulationEngine},
    types::{
        shrimp_biomass_g, PlayerAction, ShrimpRuntimeParams, SimError, StabilityTracker, TankState,
        LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
    },
};

/// Current schema version. Bump this when the save format changes.
///
/// When you bump from N to N+1, you **must** also append a migration function
/// to [`MIGRATIONS`]. See the migration contract below.
pub const SCHEMA_VERSION: u32 = 7;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Oldest schema version that the migration chain can handle.
const MIN_SUPPORTED_SCHEMA: u32 = 2;

// ---------------------------------------------------------------------------
// Migration registry
// ---------------------------------------------------------------------------

/// A migration function transforms a raw JSON `SaveFile` from version N to
/// N+1. The `&mut Value` is the top-level object; access state fields via JSON
/// pointers such as `/state/water/field_name`.
///
/// Operating on raw JSON means migrations can handle field renames, removals,
/// additions, and value transforms regardless of the current Rust struct shape.
///
/// Migrations must fail explicitly on malformed legacy payloads instead of
/// silently returning early; later phases will need that for field renames and
/// structural transforms.
type MigrationFn = fn(&mut Value) -> Result<(), SimError>;

/// Ordered list of migrations.  Index 0 migrates `MIN_SUPPORTED_SCHEMA` →
/// `MIN_SUPPORTED_SCHEMA + 1`, index 1 migrates `MIN_SUPPORTED_SCHEMA + 1` →
/// `MIN_SUPPORTED_SCHEMA + 2`, and so on.
///
/// **Invariant**: `MIGRATIONS.len() == (SCHEMA_VERSION - MIN_SUPPORTED_SCHEMA)`
///
/// # Migration contract — how to add a new migration
///
/// 1. Write a function `fn migrate_vN_to_vM(value: &mut Value) -> Result<(), SimError>` that
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
/// 5. Return a descriptive [`SimError::SchemaMigration`] when a required
///    source field is missing or has the wrong type. Do not silently skip
///    malformed legacy payloads.
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
    // Index 1: schema 3 → 4
    // Add shrimp feeding pathway parameters and animal.reserve_g. Process
    // parameters rely on serde defaults, while reserve_g is reconstructed
    // from the serialized shrimp population so migrated colonies keep the
    // retained biomass needed for post-load growth/reproduction transitions.
    migrate_v3_to_v4,
    // Index 2: schema 4 → 5
    // Transform the flat AnimalState fields (adults_count, juveniles_count,
    // condition_index, reserve_g, maturation_accum) into nested StageCohort
    // structs (juvenile, sub_adult, adult) with proportional reserve
    // distribution. Adds new molt lifecycle fields with sensible defaults.
    // Also renames the TrimPlants queued action to
    // TrimPlantsAndLeaveCuttings so legacy queued trims preserve their
    // original in-tank cuttings behavior.
    migrate_v4_to_v5,
    // Index 3: schema 5 → 6
    // Add habitat registry, geometry.hardscape_area_cm2, and filter.media_area_cm2.
    // All new fields use #[serde(default)]; habitat registry is populated in the
    // post-migration fixup.
    migrate_v5_to_v6,
    // Index 4: schema 6 → 7
    // Persist substrate colonizable_area_factor explicitly on each layer so
    // habitat refreshes preserve preset/custom substrate area scaling.
    migrate_v6_to_v7,
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
    pub fn new(mut state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        state.refresh_habitat_registry();
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
            MIGRATIONS[idx](&mut value)?;
            set_schema_version(&mut value, v + 1)?;
        }

        // Detect whether the stability tracker was serialized before we
        // deserialize (it may be absent in old saves that predate the field).
        let stability_tracker_present = has_serialized_stability_tracker(&value);

        let mut save: Self = serde_json::from_value(value)
            .map_err(|error| SimError::Deserialization(error.to_string()))?;

        let carbonate_cache_normalized = normalize_loaded_carbonate_state(&mut save.state);
        reconcile_stability_tracker(
            &mut save.state,
            file_version,
            stability_tracker_present,
            carbonate_cache_normalized,
        );

        // Treat habitat registry as derived serialized state: always rebuild it
        // on load so stale saves and migrated payloads re-enter the engine with
        // current geometry-driven values.
        save.state.refresh_habitat_registry();

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
fn migrate_v2_to_v3(value: &mut Value) -> Result<(), SimError> {
    // Compute gross and net volumes from geometry + substrate layers.
    let length = required_positive_f64_at(value, 2, 3, "/state/geometry/length_cm")?;
    let width = required_positive_f64_at(value, 2, 3, "/state/geometry/width_cm")?;
    let fill_height = required_positive_f64_at(value, 2, 3, "/state/geometry/fill_height_cm")?;
    let gross_volume_l = length * width * fill_height / 1000.0;
    if gross_volume_l <= f64::EPSILON {
        return Err(schema_migration_error(
            2,
            3,
            format!("legacy geometry yields non-positive gross water volume: {gross_volume_l} L"),
        ));
    }

    let substrate_layers = required_array_at(value, 2, 3, "/state/substrate_layers")?;
    let substrate_depth =
        substrate_layers
            .iter()
            .enumerate()
            .try_fold(0.0, |sum, (index, layer)| {
                let depth = layer
                    .get("depth_cm")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| {
                        schema_migration_error(
                            2,
                            3,
                            format!(
                                "expected finite number at /state/substrate_layers/{index}/depth_cm"
                            ),
                        )
                    })?;
                if depth < 0.0 {
                    return Err(schema_migration_error(
                        2,
                        3,
                        format!(
                            "expected non-negative number at /state/substrate_layers/{index}/depth_cm, got {depth}"
                        ),
                    ));
                }
                Ok::<_, SimError>(sum + depth)
            })?;

    if substrate_depth > fill_height + 1e-9 {
        return Err(schema_migration_error(
            2,
            3,
            format!(
                "legacy substrate depth {substrate_depth} cm exceeds fill height {fill_height} cm"
            ),
        ));
    }

    let displacement_l = length * width * substrate_depth.min(fill_height) / 1000.0;
    let net_volume_l = gross_volume_l - displacement_l;
    if net_volume_l <= f64::EPSILON {
        return Err(schema_migration_error(
            2,
            3,
            format!(
                "legacy geometry/substrate leaves non-positive net water volume: {net_volume_l} L"
            ),
        ));
    }

    let scale = net_volume_l / gross_volume_l;

    // Rescale every canonical dissolved total field in the water object.
    // `bicarbonate_mg_total` is solver-derived in the current carbonate
    // contract and is reprojected by `normalize_loaded_carbonate_state()`
    // after migrations complete, so it is intentionally excluded here.
    let water = required_object_mut_at(value, 2, 3, "/state/water")?;

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
        "chloride_mg_total",
        "sulfate_mg_total",
    ];

    for field in &dissolved_fields {
        let val = water
            .get_mut(*field)
            .ok_or_else(|| schema_migration_error(2, 3, format!("missing /state/water/{field}")))?;
        let num = val.as_f64().ok_or_else(|| {
            schema_migration_error(
                2,
                3,
                format!("expected finite number at /state/water/{field}"),
            )
        })?;
        *val = serde_json::json!(num * scale);
    }

    Ok(())
}

/// Schema 3 → 4: add shrimp feeding pathway parameters (assimilation
/// efficiency, respiration/excretion/growth fractions, O2:C quotient) and
/// `animal.reserve_g`. Process parameters still deserialize through
/// `#[serde(default)]`, but reserve_g must be reconstructed from the legacy
/// shrimp population because later growth, hatching, and maturation all draw
/// exclusively from that retained organic-matter pool.
fn migrate_v3_to_v4(value: &mut Value) -> Result<(), SimError> {
    let animal = required_object_mut_at(value, 3, 4, "/state/animal")?;

    if animal.contains_key("reserve_g") {
        return Ok(());
    }

    let adults_count = animal
        .get("adults_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_migration_error(3, 4, "expected u32 at /state/animal/adults_count"))
        .and_then(|count| {
            u32::try_from(count).map_err(|_| {
                schema_migration_error(
                    3,
                    4,
                    format!("value at /state/animal/adults_count exceeds u32 range: {count}"),
                )
            })
        })?;
    let juveniles_count = animal
        .get("juveniles_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            schema_migration_error(3, 4, "expected u32 at /state/animal/juveniles_count")
        })
        .and_then(|count| {
            u32::try_from(count).map_err(|_| {
                schema_migration_error(
                    3,
                    4,
                    format!("value at /state/animal/juveniles_count exceeds u32 range: {count}"),
                )
            })
        })?;

    let reserve_g =
        shrimp_biomass_g(adults_count, 0, juveniles_count) * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    animal.insert("reserve_g".to_string(), serde_json::json!(reserve_g));

    Ok(())
}

/// Schema 4 → 5: two structural changes.
///
/// 1. **Stage-structured shrimp model**: transform flat `AnimalState` fields
///    (`adults_count`, `juveniles_count`, `condition_index`, `reserve_g`,
///    `maturation_accum`) into nested `StageCohort` structs (`juvenile`,
///    `sub_adult`, `adult`). Reserve is distributed proportional to biomass.
///    New molt lifecycle fields (`molt_readiness`, `inter_molt_timer_days`,
///    `last_molt_success`, `failed_molt_accum`) get sensible defaults.
///
/// 2. **Action rename**: `TrimPlants` → `TrimPlantsAndLeaveCuttings` in the
///    `queued_actions` array so legacy queued trims keep their original
///    closed-system semantics.
fn migrate_v4_to_v5(value: &mut Value) -> Result<(), SimError> {
    {
        let legacy_total_maturation_days = required_positive_f64_at(
            value,
            4,
            5,
            "/state/process_params/shrimp_juvenile_maturation_days",
        )?;
        let state = required_object_mut_at(value, 4, 5, "/state")?;
        let shrimp_params = state
            .entry("shrimp_params".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let shrimp_params = shrimp_params.as_object_mut().ok_or_else(|| {
            schema_migration_error(4, 5, "expected object at /state/shrimp_params")
        })?;
        let (juvenile_to_subadult_days, subadult_to_adult_days) =
            ShrimpRuntimeParams::split_legacy_total_maturation_days(legacy_total_maturation_days);
        if !shrimp_params.contains_key("juvenile_to_subadult_days") {
            shrimp_params.insert(
                "juvenile_to_subadult_days".to_string(),
                serde_json::json!(juvenile_to_subadult_days),
            );
        }
        if !shrimp_params.contains_key("subadult_to_adult_days") {
            shrimp_params.insert(
                "subadult_to_adult_days".to_string(),
                serde_json::json!(subadult_to_adult_days),
            );
        }
    }

    // --- Stage-structured shrimp model migration ---
    //
    // Scoped block so the mutable borrow of `/state/animal` is released
    // before the queued-actions migration borrows `value` again.
    {
        let animal = required_object_mut_at(value, 4, 5, "/state/animal")?;

        // If already migrated (has nested "juvenile" key), skip animal transform.
        if !animal.contains_key("juvenile") {
            // --- Read old flat fields ---

            let adults_count = animal
                .get("adults_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    schema_migration_error(4, 5, "expected u32 at /state/animal/adults_count")
                })
                .and_then(|count| {
                    u32::try_from(count).map_err(|_| {
                        schema_migration_error(
                            4,
                            5,
                            format!(
                                "value at /state/animal/adults_count exceeds u32 range: {count}"
                            ),
                        )
                    })
                })?;

            let juveniles_count = animal
                .get("juveniles_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    schema_migration_error(4, 5, "expected u32 at /state/animal/juveniles_count")
                })
                .and_then(|count| {
                    u32::try_from(count).map_err(|_| {
                        schema_migration_error(
                            4,
                            5,
                            format!(
                                "value at /state/animal/juveniles_count exceeds u32 range: {count}"
                            ),
                        )
                    })
                })?;

            let condition_index = animal
                .get("condition_index")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    schema_migration_error(
                        4,
                        5,
                        "expected finite number at /state/animal/condition_index",
                    )
                })?;

            let reserve_g = animal
                .get("reserve_g")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    schema_migration_error(
                        4,
                        5,
                        "expected finite number at /state/animal/reserve_g",
                    )
                })?;

            let maturation_accum = animal
                .get("maturation_accum")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    schema_migration_error(
                        4,
                        5,
                        "expected finite number at /state/animal/maturation_accum",
                    )
                })?;

            // --- Compute proportional reserve distribution ---

            const ADULT_BIOMASS_G: f64 = 0.12; // ADULT_SHRIMP_BIOMASS_G
            const JUVENILE_BIOMASS_G: f64 = 0.05; // JUVENILE_SHRIMP_BIOMASS_G

            let adult_biomass = f64::from(adults_count) * ADULT_BIOMASS_G;
            let juvenile_biomass = f64::from(juveniles_count) * JUVENILE_BIOMASS_G;
            let total_biomass = adult_biomass + juvenile_biomass;

            let adult_reserve = if total_biomass > 0.0 {
                reserve_g * adult_biomass / total_biomass
            } else {
                reserve_g
            };
            let juvenile_reserve = reserve_g - adult_reserve;

            // --- Build nested stage cohort objects ---

            let juvenile_obj = serde_json::json!({
                "count": juveniles_count,
                "reserve_g": juvenile_reserve,
                "condition_index": condition_index,
                "maturation_accum": maturation_accum,
            });

            let sub_adult_obj = serde_json::json!({
                "count": 0,
                "reserve_g": 0.0,
                "condition_index": condition_index,
                "maturation_accum": 0.0,
            });

            let adult_obj = serde_json::json!({
                "count": adults_count,
                "reserve_g": adult_reserve,
                "condition_index": condition_index,
                "maturation_accum": 0.0,
            });

            // --- Remove old flat fields ---

            animal.remove("adults_count");
            animal.remove("juveniles_count");
            animal.remove("condition_index");
            animal.remove("reserve_g");
            animal.remove("maturation_accum");

            // --- Insert new nested fields and molt lifecycle defaults ---

            animal.insert("juvenile".to_string(), juvenile_obj);
            animal.insert("sub_adult".to_string(), sub_adult_obj);
            animal.insert("adult".to_string(), adult_obj);

            // New molt lifecycle fields with sensible defaults.
            if !animal.contains_key("molt_readiness") {
                animal.insert("molt_readiness".to_string(), serde_json::json!(0.5));
            }
            if !animal.contains_key("inter_molt_timer_days") {
                animal.insert("inter_molt_timer_days".to_string(), serde_json::json!(14.0));
            }
            if !animal.contains_key("last_molt_success") {
                animal.insert("last_molt_success".to_string(), serde_json::json!(true));
            }
            if !animal.contains_key("failed_molt_accum") {
                animal.insert("failed_molt_accum".to_string(), serde_json::json!(0.0));
            }
        }
    }

    // --- Migrate queued actions: legacy TrimPlants left cuttings in-tank ---

    if let Some(actions) = value
        .get_mut("queued_actions")
        .and_then(Value::as_array_mut)
    {
        for action in actions.iter_mut() {
            if let Some(obj) = action.as_object_mut() {
                if let Some(inner) = obj.remove("TrimPlants") {
                    obj.insert("TrimPlantsAndLeaveCuttings".to_string(), inner);
                }
            }
        }
    }

    Ok(())
}

/// Schema 5 → 6: add habitat registry, geometry.hardscape_area_cm2, and
/// filter.media_area_cm2. All new fields carry `#[serde(default)]` so no
/// JSON transform is required.
fn migrate_v5_to_v6(_value: &mut Value) -> Result<(), SimError> {
    Ok(())
}

/// Schema 6 → 7: persist substrate.colonizable_area_factor explicitly.
fn migrate_v6_to_v7(value: &mut Value) -> Result<(), SimError> {
    let substrate_layers = value
        .pointer_mut("/state/substrate_layers")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| schema_migration_error(6, 7, "expected array at /state/substrate_layers"))?;

    for (index, layer) in substrate_layers.iter_mut().enumerate() {
        let layer_obj = layer.as_object_mut().ok_or_else(|| {
            schema_migration_error(
                6,
                7,
                format!("expected object at /state/substrate_layers/{index}"),
            )
        })?;
        let kind = layer_obj
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                schema_migration_error(
                    6,
                    7,
                    format!("expected string at /state/substrate_layers/{index}/kind"),
                )
            })?;
        let factor = match kind {
            "InertSand" => 0.5,
            "InertGravel" => 0.6,
            "ActivePlanted" => 0.8,
            "CoarsePorous" => 0.9,
            other => {
                return Err(schema_migration_error(
                    6,
                    7,
                    format!(
                        "unsupported substrate kind `{other}` at /state/substrate_layers/{index}/kind"
                    ),
                ));
            }
        };
        layer_obj.insert(
            "colonizable_area_factor".to_string(),
            Value::from(factor),
        );
    }

    Ok(())
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

fn normalize_loaded_carbonate_state(state: &mut TankState) -> bool {
    let original_ph = state.water.ph;
    let original_bicarbonate_mg_total = state.water.bicarbonate_mg_total;
    let volume_l = state.water_volume_l();

    crate::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);

    (state.water.ph - original_ph).abs() > 1e-9
        || (state.water.bicarbonate_mg_total - original_bicarbonate_mg_total).abs() > 1e-6
}

fn reconcile_stability_tracker(
    state: &mut TankState,
    source_schema_version: u32,
    stability_tracker_present: bool,
    carbonate_cache_normalized: bool,
) {
    if !stability_tracker_present
        || has_unseeded_legacy_stability_tracker(source_schema_version, state)
    {
        state.reseed_stability_tracker();
    } else if carbonate_cache_normalized {
        // Avoid a fake load-time chemistry swing when we repaired stale
        // cached pH/bicarbonate fields from the canonical carbonate inputs.
        state.stability_tracker.prev_ph = state.water.ph;
    }
}

/// Schema-2 saves created before tracker seeding existed can serialize the
/// `StabilityTracker::default()` sentinel even when the live water baselines
/// were different. Reseed only that legacy/default case so valid historical
/// trackers from later schemas keep their recorded baselines.
fn has_unseeded_legacy_stability_tracker(source_schema_version: u32, state: &TankState) -> bool {
    source_schema_version == 2
        && state.stability_tracker == StabilityTracker::default()
        && !stability_tracker_matches_water(state)
}

fn stability_tracker_matches_water(state: &TankState) -> bool {
    let volume_l = state.water_volume_l();
    approx_eq(
        state.stability_tracker.prev_temp_c,
        state.water.temperature_c,
        1e-9,
    ) && approx_eq(state.stability_tracker.prev_ph, state.water.ph, 1e-9)
        && approx_eq(
            state.stability_tracker.prev_gh_d,
            state.water.gh_d(volume_l),
            1e-9,
        )
        && approx_eq(
            state.stability_tracker.prev_do_mg_l,
            state.water.do_mg_per_l(volume_l),
            1e-9,
        )
}

fn approx_eq(lhs: f64, rhs: f64, tolerance: f64) -> bool {
    (lhs - rhs).abs() <= tolerance
}

fn set_schema_version(value: &mut Value, version: u32) -> Result<(), SimError> {
    let obj = value.as_object_mut().ok_or_else(|| {
        schema_migration_error(version, version, "top-level save payload must be an object")
    })?;
    obj.insert("schema_version".to_string(), Value::Number(version.into()));
    Ok(())
}

fn required_f64_at(
    value: &Value,
    from: u32,
    to: u32,
    pointer: &'static str,
) -> Result<f64, SimError> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            schema_migration_error(from, to, format!("expected finite number at {pointer}"))
        })
}

fn required_positive_f64_at(
    value: &Value,
    from: u32,
    to: u32,
    pointer: &'static str,
) -> Result<f64, SimError> {
    let number = required_f64_at(value, from, to, pointer)?;
    if number <= 0.0 {
        return Err(schema_migration_error(
            from,
            to,
            format!("expected positive number at {pointer}, got {number}"),
        ));
    }
    Ok(number)
}

fn required_array_at<'a>(
    value: &'a Value,
    from: u32,
    to: u32,
    pointer: &'static str,
) -> Result<&'a Vec<Value>, SimError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| schema_migration_error(from, to, format!("expected array at {pointer}")))
}

fn required_object_mut_at<'a>(
    value: &'a mut Value,
    from: u32,
    to: u32,
    pointer: &'static str,
) -> Result<&'a mut serde_json::Map<String, Value>, SimError> {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| schema_migration_error(from, to, format!("expected object at {pointer}")))
}

fn schema_migration_error(from: u32, to: u32, message: impl Into<String>) -> SimError {
    SimError::SchemaMigration {
        from,
        to,
        message: message.into(),
    }
}
