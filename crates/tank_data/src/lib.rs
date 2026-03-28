mod presets;

use serde::de::DeserializeOwned;
use thiserror::Error;

pub use presets::{
    PlantPreset, ProcessParamsPreset, Provenance, ScenarioPreset, ShrimpPreset, SourceWaterPreset,
    SubstratePreset,
};
// Re-export core provenance types for convenience.
pub use tank_core::types::provenance::{
    check_all_ranges, check_param_range, format_param, ConfidenceLevel, ParamMeta, RangeWarning,
};

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("unknown preset `{id}` in category `{category}`")]
    UnknownPreset { category: &'static str, id: String },
    #[error("failed to parse preset `{id}` in category `{category}`: {message}")]
    Parse {
        category: &'static str,
        id: String,
        message: String,
    },
    #[error("validation failed for preset `{id}` in category `{category}`: {message}")]
    Validation {
        category: &'static str,
        id: String,
        message: String,
    },
}

pub fn load_source_water(id: &str) -> Result<SourceWaterPreset, PresetError> {
    let preset: SourceWaterPreset = load_from_registry("source_water", id, SOURCE_WATER_PRESETS)?;
    preset
        .validate()
        .map_err(|message| PresetError::Validation {
            category: "source_water",
            id: id.to_string(),
            message,
        })?;
    Ok(preset)
}

pub fn load_substrate(id: &str) -> Result<SubstratePreset, PresetError> {
    load_from_registry("substrate", id, SUBSTRATE_PRESETS)
}

pub fn load_plant(id: &str) -> Result<PlantPreset, PresetError> {
    load_from_registry("plants", id, PLANT_PRESETS)
}

pub fn load_shrimp(id: &str) -> Result<ShrimpPreset, PresetError> {
    let preset: ShrimpPreset = load_from_registry("shrimp", id, SHRIMP_PRESETS)?;
    preset
        .validate()
        .map_err(|message| PresetError::Validation {
            category: "shrimp",
            id: id.to_string(),
            message,
        })?;
    Ok(preset)
}

pub fn load_process_params(id: &str) -> Result<ProcessParamsPreset, PresetError> {
    let preset: ProcessParamsPreset = load_from_registry("process", id, PROCESS_PRESETS)?;
    preset
        .validate()
        .map_err(|message| PresetError::Validation {
            category: "process",
            id: id.to_string(),
            message,
        })?;
    Ok(preset)
}

pub fn load_scenario(id: &str) -> Result<ScenarioPreset, PresetError> {
    load_from_registry("scenarios", id, SCENARIO_PRESETS)
}

pub fn source_water_ids() -> &'static [&'static str] {
    &["soft_acidic", "moderate", "hard_shrimp", "ro_like"]
}

pub fn substrate_ids() -> &'static [&'static str] {
    &[
        "inert_sand",
        "inert_gravel",
        "active_planted",
        "coarse_porous",
    ]
}

pub fn plant_ids() -> &'static [&'static str] {
    &["fast_stem", "root_rosette"]
}

pub fn shrimp_ids() -> &'static [&'static str] {
    &["neocaridina_davidi"]
}

pub fn process_ids() -> &'static [&'static str] {
    &["default"]
}

pub fn scenario_ids() -> &'static [&'static str] {
    &["nano_cycle", "medium_planted", "warm_room"]
}

fn load_from_registry<T>(
    category: &'static str,
    id: &str,
    registry: &[(&str, &str)],
) -> Result<T, PresetError>
where
    T: DeserializeOwned,
{
    let Some((_, raw)) = registry.iter().find(|(entry_id, _)| *entry_id == id) else {
        return Err(PresetError::UnknownPreset {
            category,
            id: id.to_string(),
        });
    };

    toml::from_str(raw).map_err(|error| PresetError::Parse {
        category,
        id: id.to_string(),
        message: error.to_string(),
    })
}

const SOURCE_WATER_PRESETS: &[(&str, &str)] = &[
    (
        "soft_acidic",
        include_str!("../data/source_water/soft_acidic.toml"),
    ),
    (
        "moderate",
        include_str!("../data/source_water/moderate.toml"),
    ),
    (
        "hard_shrimp",
        include_str!("../data/source_water/hard_shrimp.toml"),
    ),
    ("ro_like", include_str!("../data/source_water/ro_like.toml")),
];

const SUBSTRATE_PRESETS: &[(&str, &str)] = &[
    (
        "inert_sand",
        include_str!("../data/substrate/inert_sand.toml"),
    ),
    (
        "inert_gravel",
        include_str!("../data/substrate/inert_gravel.toml"),
    ),
    (
        "active_planted",
        include_str!("../data/substrate/active_planted.toml"),
    ),
    (
        "coarse_porous",
        include_str!("../data/substrate/coarse_porous.toml"),
    ),
];

const PLANT_PRESETS: &[(&str, &str)] = &[
    ("fast_stem", include_str!("../data/plants/fast_stem.toml")),
    (
        "root_rosette",
        include_str!("../data/plants/root_rosette.toml"),
    ),
];

const SHRIMP_PRESETS: &[(&str, &str)] = &[(
    "neocaridina_davidi",
    include_str!("../data/shrimp/neocaridina_davidi.toml"),
)];

const PROCESS_PRESETS: &[(&str, &str)] =
    &[("default", include_str!("../data/process/default.toml"))];

const SCENARIO_PRESETS: &[(&str, &str)] = &[
    (
        "nano_cycle",
        include_str!("../data/scenarios/nano_cycle.toml"),
    ),
    (
        "medium_planted",
        include_str!("../data/scenarios/medium_planted.toml"),
    ),
    (
        "warm_room",
        include_str!("../data/scenarios/warm_room.toml"),
    ),
];
