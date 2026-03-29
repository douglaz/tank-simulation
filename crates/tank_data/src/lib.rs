mod presets;

use presets::parse_process_params_preset;
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
    let (preset, diagnostics) = load_source_water_with_diagnostics(id)?;
    emit_diagnostics(&diagnostics);
    Ok(preset)
}

pub fn load_substrate(id: &str) -> Result<SubstratePreset, PresetError> {
    let (preset, diagnostics) = load_substrate_with_diagnostics(id)?;
    emit_diagnostics(&diagnostics);
    Ok(preset)
}

pub fn load_plant(id: &str) -> Result<PlantPreset, PresetError> {
    let (preset, diagnostics) = load_plant_with_diagnostics(id)?;
    emit_diagnostics(&diagnostics);
    Ok(preset)
}

pub fn load_shrimp(id: &str) -> Result<ShrimpPreset, PresetError> {
    let (preset, diagnostics) = load_shrimp_with_diagnostics(id)?;
    emit_diagnostics(&diagnostics);
    Ok(preset)
}

pub fn load_process_params(id: &str) -> Result<ProcessParamsPreset, PresetError> {
    let (preset, diagnostics) = load_process_params_with_diagnostics(id)?;
    emit_diagnostics(&diagnostics);
    Ok(preset)
}

pub fn load_source_water_with_diagnostics(
    id: &str,
) -> Result<(SourceWaterPreset, Vec<String>), PresetError> {
    load_validated_with_range_diagnostics(
        "source_water",
        id,
        SOURCE_WATER_PRESETS,
        SourceWaterPreset::validate,
        SourceWaterPreset::check_ranges,
    )
}

pub fn load_substrate_with_diagnostics(
    id: &str,
) -> Result<(SubstratePreset, Vec<String>), PresetError> {
    load_validated_with_range_diagnostics(
        "substrate",
        id,
        SUBSTRATE_PRESETS,
        SubstratePreset::validate,
        SubstratePreset::check_ranges,
    )
}

pub fn load_plant_with_diagnostics(id: &str) -> Result<(PlantPreset, Vec<String>), PresetError> {
    load_validated_with_range_diagnostics(
        "plants",
        id,
        PLANT_PRESETS,
        PlantPreset::validate,
        PlantPreset::check_ranges,
    )
}

pub fn load_shrimp_with_diagnostics(id: &str) -> Result<(ShrimpPreset, Vec<String>), PresetError> {
    load_validated_with_range_diagnostics(
        "shrimp",
        id,
        SHRIMP_PRESETS,
        ShrimpPreset::validate,
        ShrimpPreset::check_ranges,
    )
}

pub fn load_process_params_with_diagnostics(
    id: &str,
) -> Result<(ProcessParamsPreset, Vec<String>), PresetError> {
    load_parsed_validated_with_range_diagnostics(
        "process",
        id,
        PROCESS_PRESETS,
        parse_process_params_preset,
        ProcessParamsPreset::validate,
        ProcessParamsPreset::check_ranges,
    )
}

pub fn load_scenario(id: &str) -> Result<ScenarioPreset, PresetError> {
    load_from_registry("scenarios", id, SCENARIO_PRESETS)
}

pub fn source_water_ids() -> &'static [&'static str] {
    &[
        "soft_acidic",
        "moderate",
        "moderate_planted",
        "hard_shrimp",
        "ro_like",
    ]
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

fn load_validated_with_range_diagnostics<T, V, W>(
    category: &'static str,
    id: &str,
    registry: &[(&str, &str)],
    validate: V,
    check_ranges: W,
) -> Result<(T, Vec<String>), PresetError>
where
    T: DeserializeOwned,
    V: Fn(&T) -> Result<(), String>,
    W: Fn(&T) -> Vec<RangeWarning>,
{
    load_parsed_validated_with_range_diagnostics(
        category,
        id,
        registry,
        |raw| toml::from_str(raw),
        validate,
        check_ranges,
    )
}

fn load_parsed_validated_with_range_diagnostics<T, P, E, V, W>(
    category: &'static str,
    id: &str,
    registry: &[(&str, &str)],
    parse: P,
    validate: V,
    check_ranges: W,
) -> Result<(T, Vec<String>), PresetError>
where
    P: Fn(&str) -> Result<T, E>,
    E: std::fmt::Display,
    V: Fn(&T) -> Result<(), String>,
    W: Fn(&T) -> Vec<RangeWarning>,
{
    let Some((_, raw)) = registry.iter().find(|(entry_id, _)| *entry_id == id) else {
        return Err(PresetError::UnknownPreset {
            category,
            id: id.to_string(),
        });
    };

    let preset = parse(raw).map_err(|error| PresetError::Parse {
        category,
        id: id.to_string(),
        message: error.to_string(),
    })?;

    validate(&preset).map_err(|message| PresetError::Validation {
        category,
        id: id.to_string(),
        message,
    })?;
    let diagnostics = format_range_diagnostics(category, id, &check_ranges(&preset));
    Ok((preset, diagnostics))
}

fn format_range_diagnostics(
    category: &'static str,
    id: &str,
    warnings: &[RangeWarning],
) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| format!("warning: preset `{id}` in category `{category}`: {warning}"))
        .collect()
}

fn emit_diagnostics(diagnostics: &[String]) {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
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
        "moderate_planted",
        include_str!("../data/source_water/moderate_planted.toml"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_range_diagnostics_use_explicit_warning_lines() -> Result<(), PresetError> {
        let registry = &[(
            "test",
            r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.08
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.12
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0
aob_k_tan_mg_n_per_l = 200.0

[param_meta.aob_k_tan_mg_n_per_l]
unit = "mg N/L"
valid_range = [0.1, 5.0]
"#,
        )];

        let (_preset, diagnostics) =
            load_validated_with_range_diagnostics::<ProcessParamsPreset, _, _>(
                "process",
                "test",
                registry,
                ProcessParamsPreset::validate,
                ProcessParamsPreset::check_ranges,
            )?;

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].contains("warning: preset `test` in category `process`"),
            "diagnostic should identify the preset: {}",
            diagnostics[0]
        );
        assert!(
            diagnostics[0].contains("aob_k_tan_mg_n_per_l"),
            "diagnostic should name the offending parameter: {}",
            diagnostics[0]
        );
        assert!(
            diagnostics[0].contains("200"),
            "diagnostic should include the out-of-range value: {}",
            diagnostics[0]
        );
        assert!(
            diagnostics[0].contains("0.1") && diagnostics[0].contains("5"),
            "diagnostic should include the valid range bounds: {}",
            diagnostics[0]
        );
        Ok(())
    }

    #[test]
    fn process_loader_migrates_legacy_kinetic_keys_before_range_checks() -> Result<(), PresetError>
    {
        let registry = &[(
            "legacy",
            r#"
id = "legacy"
name = "Legacy"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.08
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.12
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0
aob_k_tan_mg = 200.0

[param_meta.aob_k_tan_mg]
unit = "mg N/L"
valid_range = [0.1, 5.0]
"#,
        )];

        let (preset, diagnostics) = load_parsed_validated_with_range_diagnostics(
            "process",
            "legacy",
            registry,
            parse_process_params_preset,
            ProcessParamsPreset::validate,
            ProcessParamsPreset::check_ranges,
        )?;

        assert_eq!(preset.aob_k_tan_mg_n_per_l, 10.0);
        assert!(preset.param_meta.contains_key("aob_k_tan_mg_n_per_l"));
        assert!(!preset.param_meta.contains_key("aob_k_tan_mg"));
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("parameter `aob_k_tan_mg_n_per_l` value 10"));
        Ok(())
    }

    #[test]
    fn shipped_param_meta_loaders_return_no_diagnostics() -> Result<(), PresetError> {
        assert!(load_source_water_with_diagnostics("moderate")?.1.is_empty());
        assert!(load_substrate_with_diagnostics("active_planted")?
            .1
            .is_empty());
        assert!(load_plant_with_diagnostics("fast_stem")?.1.is_empty());
        assert!(load_shrimp_with_diagnostics("neocaridina_davidi")?
            .1
            .is_empty());
        assert!(load_process_params_with_diagnostics("default")?
            .1
            .is_empty());
        Ok(())
    }
}
