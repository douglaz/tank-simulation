//! Unit tests for geometry-aware scaling of hardware, plant mass, and stocking.
//!
//! These tests verify the scaling constants and formulas added in tanksim-6e5.5.6:
//! - Filter flow = volume_l × 10 (turnovers/hr)
//! - Heater watts = volume_l × 0.75
//! - Plant biomass = footprint_cm² × 3.0 / 1000 per guild
//! - Auto-stocking = volume_l × 0.2 adults/L
//! - Microbe scaling ∝ footprint area (floored at 1×)

use tank_core::SimSeed;
use tank_scenarios::{
    seeded_state_with_full_overrides, ScenarioGeometryOverrides, StartupHeaterPreset,
    StartupLightPreset, StartupOverrides, StartupPlantSelection, StartupSubstratePreset,
    AUTO_STOCK_ADULTS_PER_LITER, FILTER_FLOW_TURNOVERS_PER_HOUR, HEATER_WATTS_PER_LITER,
    PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT,
};

/// Helper: build a state with a given size_scale on medium_planted and return it.
fn medium_planted_at_scale(size_scale: f64) -> tank_core::TankState {
    seeded_state_with_full_overrides(
        SimSeed(100),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale,
                fill_ratio: 1.0,
            },
            ..StartupOverrides::default()
        },
    )
    .expect("medium_planted should materialize")
}

/// Helper: build a state with a given size_scale on nano_cycle and return it.
fn nano_at_scale(size_scale: f64) -> tank_core::TankState {
    seeded_state_with_full_overrides(
        SimSeed(100),
        "nano_cycle",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale,
                fill_ratio: 1.0,
            },
            ..StartupOverrides::default()
        },
    )
    .expect("nano_cycle should materialize")
}

// ---------------------------------------------------------------------------
// Filter flow scaling
// ---------------------------------------------------------------------------

#[test]
fn filter_flow_scales_with_volume_at_10x_turnover() -> Result<(), Box<dyn std::error::Error>> {
    // medium_planted base: 60×30×36, fill 32 → volume ≈ 57.6L
    let state_1x = medium_planted_at_scale(1.0);
    let volume_1x = state_1x.geometry.gross_water_volume_l();
    let expected_flow_1x = volume_1x * FILTER_FLOW_TURNOVERS_PER_HOUR;

    assert!(
        (state_1x.hardware.filter.flow_lph - expected_flow_1x).abs() < 0.01,
        "1× filter flow should be volume × 10: expected {expected_flow_1x:.1}, got {:.1}",
        state_1x.hardware.filter.flow_lph
    );

    // 2× scale: all dimensions double, volume goes 8×
    let state_2x = medium_planted_at_scale(2.0);
    let volume_2x = state_2x.geometry.gross_water_volume_l();
    let expected_flow_2x = volume_2x * FILTER_FLOW_TURNOVERS_PER_HOUR;

    assert!(
        (state_2x.hardware.filter.flow_lph - expected_flow_2x).abs() < 0.01,
        "2× filter flow should be volume × 10: expected {expected_flow_2x:.1}, got {:.1}",
        state_2x.hardware.filter.flow_lph
    );

    // Volume ratio should be 8× (2³)
    let volume_ratio = volume_2x / volume_1x;
    assert!(
        (volume_ratio - 8.0).abs() < 0.01,
        "2× geometry should have 8× volume: ratio={volume_ratio:.3}"
    );

    // Flow ratio should also be 8×
    let flow_ratio = state_2x.hardware.filter.flow_lph / state_1x.hardware.filter.flow_lph;
    assert!(
        (flow_ratio - 8.0).abs() < 0.01,
        "filter flow should scale 8× with 2× geometry: ratio={flow_ratio:.3}"
    );

    Ok(())
}

/// AC: 100L tank should have filter_flow_lph ≈ 1000.
#[test]
fn filter_flow_approximately_1000_for_100l_tank() -> Result<(), Box<dyn std::error::Error>> {
    // medium_planted base volume ≈ 57.6L. To get ~100L, use scale ≈ 1.2.
    // Exact: scale³ × 57.6 = 100 → scale = (100/57.6)^(1/3) ≈ 1.2016
    let scale = (100.0_f64 / 57.6).cbrt();
    let state = medium_planted_at_scale(scale);
    let volume_l = state.geometry.gross_water_volume_l();
    let flow = state.hardware.filter.flow_lph;

    assert!(
        (volume_l - 100.0).abs() < 1.0,
        "tank volume should be ~100L, got {volume_l:.1}L"
    );
    assert!(
        (flow - 1000.0).abs() < 15.0,
        "filter flow should be ~1000 lph for 100L tank, got {flow:.1}"
    );

    Ok(())
}

/// AC: 10L tank should have filter_flow_lph ≈ 100.
#[test]
fn filter_flow_approximately_100_for_10l_tank() -> Result<(), Box<dyn std::error::Error>> {
    // nano_cycle base: 30×20×20, fill 18 → volume ≈ 10.8L
    let state = nano_at_scale(1.0);
    let volume_l = state.geometry.gross_water_volume_l();
    let flow = state.hardware.filter.flow_lph;

    assert!(
        (volume_l - 10.8).abs() < 0.5,
        "nano tank volume should be ~10.8L, got {volume_l:.1}L"
    );
    assert!(
        (flow - 108.0).abs() < 1.0,
        "filter flow should be ~108 lph for 10.8L tank, got {flow:.1}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Heater wattage scaling
// ---------------------------------------------------------------------------

#[test]
fn heater_watts_scales_with_volume() -> Result<(), Box<dyn std::error::Error>> {
    let state_1x = medium_planted_at_scale(1.0);
    let volume_1x = state_1x.geometry.gross_water_volume_l();
    let expected_watts_1x = volume_1x * HEATER_WATTS_PER_LITER;

    assert!(
        (state_1x.hardware.heater.max_watts - expected_watts_1x).abs() < 0.01,
        "heater should be volume × 0.75: expected {expected_watts_1x:.1}W, got {:.1}W",
        state_1x.hardware.heater.max_watts
    );

    // AC: 100L → ~75W, 10L → ~7.5W
    let scale_100l = (100.0_f64 / 57.6).cbrt();
    let state_100l = medium_planted_at_scale(scale_100l);
    assert!(
        (state_100l.hardware.heater.max_watts - 75.0).abs() < 2.0,
        "100L heater should be ~75W, got {:.1}W",
        state_100l.hardware.heater.max_watts
    );

    let state_nano = nano_at_scale(1.0);
    assert!(
        (state_nano.hardware.heater.max_watts - 8.1).abs() < 1.0,
        "10.8L heater should be ~8.1W, got {:.1}W",
        state_nano.hardware.heater.max_watts
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Plant biomass scaling with footprint
// ---------------------------------------------------------------------------

#[test]
fn plant_biomass_scales_with_substrate_footprint() -> Result<(), Box<dyn std::error::Error>> {
    let state_1x = medium_planted_at_scale(1.0);
    let footprint_1x = state_1x.geometry.footprint_area_cm2();
    let expected_per_guild =
        (footprint_1x * PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT / 1000.0).max(1.0);

    for guild in &state_1x.plant_guilds {
        assert!(
            (guild.biomass_g - expected_per_guild).abs() < 0.01,
            "plant biomass should be footprint×k/1000: expected {expected_per_guild:.2}g, got {:.2}g for {:?}",
            guild.biomass_g, guild.guild
        );
    }

    // 2× geometry: footprint scales 4×, so biomass should be 4×
    let state_2x = medium_planted_at_scale(2.0);
    let footprint_2x = state_2x.geometry.footprint_area_cm2();
    let footprint_ratio = footprint_2x / footprint_1x;
    assert!(
        (footprint_ratio - 4.0).abs() < 0.01,
        "2× geometry should have 4× footprint: ratio={footprint_ratio:.3}"
    );

    for guild in &state_2x.plant_guilds {
        let expected = (footprint_2x * PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT / 1000.0).max(1.0);
        assert!(
            (guild.biomass_g - expected).abs() < 0.01,
            "2× plant biomass should be 4× per guild: expected {expected:.2}g, got {:.2}g",
            guild.biomass_g
        );
    }

    Ok(())
}

#[test]
fn nano_plant_biomass_uses_small_footprint() -> Result<(), Box<dyn std::error::Error>> {
    let state = nano_at_scale(1.0);
    // nano_cycle footprint: 30×20 = 600 cm²
    let footprint = state.geometry.footprint_area_cm2();
    assert!(
        (footprint - 600.0).abs() < 1.0,
        "nano footprint should be 600 cm², got {footprint:.1}"
    );

    let expected_biomass = (600.0 * PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT / 1000.0).max(1.0);
    assert!(
        expected_biomass < 2.0,
        "nano plant biomass per guild should be < 2g, got {expected_biomass:.2}g"
    );

    for guild in &state.plant_guilds {
        assert!(
            (guild.biomass_g - expected_biomass).abs() < 0.01,
            "nano plant biomass should be {expected_biomass:.2}g, got {:.2}g",
            guild.biomass_g
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Explicit override preservation
// ---------------------------------------------------------------------------

#[test]
fn explicit_overrides_preserved_over_auto_scaling() -> Result<(), Box<dyn std::error::Error>> {
    // Explicit overrides should win over geometry-scaled defaults
    let state = seeded_state_with_full_overrides(
        SimSeed(200),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale: 2.0,
                fill_ratio: 1.0,
            },
            source_water_profile_id: Some("moderate".to_string()),
            substrate_preset: Some(StartupSubstratePreset::InertSand),
            plant_selection: Some(StartupPlantSelection::FastStemOnly),
            filter_enabled: Some(true),
            light_preset: Some(StartupLightPreset::Hours10),
            heater_preset: Some(StartupHeaterPreset::Celsius26),
            aeration_enabled: Some(true),
            initial_adult_shrimp_count: Some(15),
            ..StartupOverrides::default()
        },
    )?;

    // Explicit shrimp count should win over auto-stocking
    assert_eq!(
        state.animal.adult.count, 15,
        "explicit shrimp count should be preserved"
    );

    // Hardware should be geometry-scaled (no explicit hardware value overrides exist)
    let volume_l = state.geometry.gross_water_volume_l();
    assert!(
        (state.hardware.filter.flow_lph - volume_l * FILTER_FLOW_TURNOVERS_PER_HOUR).abs() < 0.1,
        "filter flow should be geometry-scaled"
    );

    // Light/heater overrides should be applied
    assert_eq!(state.hardware.light.photoperiod_hours, 10.0);
    assert!(state.hardware.heater.enabled);
    assert!((state.hardware.heater.setpoint_c - 26.0).abs() < f64::EPSILON);

    Ok(())
}

// ---------------------------------------------------------------------------
// Auto-stocking
// ---------------------------------------------------------------------------

#[test]
fn auto_stock_shrimp_scales_with_volume() -> Result<(), Box<dyn std::error::Error>> {
    // auto_stock_shrimp = true, no explicit count → auto-stock at 0.2 adults/L
    let state = seeded_state_with_full_overrides(
        SimSeed(300),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale: 1.0,
                fill_ratio: 1.0,
            },
            auto_stock_shrimp: true,
            // No explicit shrimp count
            initial_adult_shrimp_count: None,
            ..StartupOverrides::default()
        },
    )?;

    let volume_l = state.geometry.gross_water_volume_l();
    let expected_count = (volume_l * AUTO_STOCK_ADULTS_PER_LITER).round() as u32;

    assert_eq!(
        state.animal.adult.count, expected_count,
        "auto-stocked count should be volume_l × 0.2 rounded: expected {expected_count} for {volume_l:.1}L"
    );

    // With 2× geometry (8× volume), should get 8× as many shrimp
    let state_2x = seeded_state_with_full_overrides(
        SimSeed(300),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale: 2.0,
                fill_ratio: 1.0,
            },
            auto_stock_shrimp: true,
            initial_adult_shrimp_count: None,
            ..StartupOverrides::default()
        },
    )?;

    let volume_2x = state_2x.geometry.gross_water_volume_l();
    let expected_2x = (volume_2x * AUTO_STOCK_ADULTS_PER_LITER).round() as u32;

    assert_eq!(
        state_2x.animal.adult.count, expected_2x,
        "2× auto-stocked count should be {expected_2x} for {volume_2x:.1}L"
    );
    assert!(
        state_2x.animal.adult.count > state.animal.adult.count * 4,
        "2× geometry should have >4× shrimp (8× volume): {} vs {}",
        state_2x.animal.adult.count,
        state.animal.adult.count
    );

    Ok(())
}

#[test]
fn explicit_count_overrides_auto_stock() -> Result<(), Box<dyn std::error::Error>> {
    // Even with auto_stock_shrimp = true, explicit count wins
    let state = seeded_state_with_full_overrides(
        SimSeed(301),
        "medium_planted",
        StartupOverrides {
            auto_stock_shrimp: true,
            initial_adult_shrimp_count: Some(3),
            ..StartupOverrides::default()
        },
    )?;

    assert_eq!(
        state.animal.adult.count, 3,
        "explicit count should override auto-stocking"
    );

    Ok(())
}

#[test]
fn no_auto_stock_when_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let state = seeded_state_with_full_overrides(
        SimSeed(302),
        "nano_cycle",
        StartupOverrides {
            auto_stock_shrimp: false,
            initial_adult_shrimp_count: None,
            ..StartupOverrides::default()
        },
    )?;

    assert_eq!(
        state.animal.adult.count, 0,
        "no shrimp when auto_stock disabled and no explicit count"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Microbe scaling with footprint
// ---------------------------------------------------------------------------

#[test]
fn microbe_biomass_scales_with_footprint_for_large_tanks() -> Result<(), Box<dyn std::error::Error>>
{
    let state_1x = medium_planted_at_scale(1.0);
    let state_2x = medium_planted_at_scale(2.0);

    let footprint_1x = state_1x.geometry.footprint_area_cm2();
    let footprint_2x = state_2x.geometry.footprint_area_cm2();
    let area_ratio = footprint_2x / footprint_1x;

    // Microbe biomass should scale proportionally with footprint
    let aob_ratio =
        state_2x.microbe.ammonia_oxidizer_biomass_g / state_1x.microbe.ammonia_oxidizer_biomass_g;
    let nob_ratio =
        state_2x.microbe.nitrite_oxidizer_biomass_g / state_1x.microbe.nitrite_oxidizer_biomass_g;
    let decomp_ratio =
        state_2x.microbe.decomposer_biomass_g / state_1x.microbe.decomposer_biomass_g;

    assert!(
        (aob_ratio - area_ratio).abs() < 0.1,
        "AOB should scale ~{area_ratio:.1}× with 2× geometry: ratio={aob_ratio:.2}"
    );
    assert!(
        (nob_ratio - area_ratio).abs() < 0.1,
        "NOB should scale ~{area_ratio:.1}× with 2× geometry: ratio={nob_ratio:.2}"
    );
    assert!(
        (decomp_ratio - area_ratio).abs() < 0.1,
        "decomposers should scale ~{area_ratio:.1}× with 2× geometry: ratio={decomp_ratio:.2}"
    );

    Ok(())
}

#[test]
fn small_tank_microbes_use_defaults() -> Result<(), Box<dyn std::error::Error>> {
    // nano_cycle footprint: 600 cm² < reference 1000 cm², so scale = 1.0 (no reduction)
    let state = nano_at_scale(1.0);

    // Default MicrobeState values
    assert!(
        (state.microbe.ammonia_oxidizer_biomass_g - 0.05).abs() < f64::EPSILON,
        "small tank AOB should use default: {:.4}",
        state.microbe.ammonia_oxidizer_biomass_g
    );
    assert!(
        (state.microbe.nitrite_oxidizer_biomass_g - 0.05).abs() < f64::EPSILON,
        "small tank NOB should use default: {:.4}",
        state.microbe.nitrite_oxidizer_biomass_g
    );
    assert!(
        (state.microbe.comammox_biomass_g - 0.01).abs() < f64::EPSILON,
        "small tank comammox should use default: {:.4}",
        state.microbe.comammox_biomass_g
    );

    Ok(())
}
