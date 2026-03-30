//! Unit tests for geometry-aware scaling of hardware, plant mass, and stocking.
//!
//! These tests verify the scaling constants and formulas added in tanksim-6e5.5.6:
//! - Filter flow = volume_l × 10 (turnovers/hr)
//! - Heater watts = max(volume_l × 0.75, geometry_heat_loss)
//! - Plant biomass = footprint_cm² × 3.0 / 1000 per guild
//! - Auto-stocking = volume_l × 0.2 adults/L
//! - Microbe scaling ∝ footprint area (floored at 1×)

use tank_core::{Engine, ProcessParams, SimSeed, SimulationEngine, TankGeometry, TankState};
use tank_scenarios::{
    recommended_auto_stock_adults_per_liter, recommended_heater_max_watts,
    seeded_state_with_full_overrides, ScenarioGeometryOverrides, StartupHeaterPreset,
    StartupLightPreset, StartupOverrides, StartupPlantSelection, StartupSubstratePreset,
    AUTO_STOCK_MAX_ADULTS_PER_LITER, AUTO_STOCK_MIN_ADULTS_PER_LITER,
    FILTER_FLOW_TURNOVERS_PER_HOUR, HEATER_MAX_WATTS_PER_LITER,
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
fn heater_watts_follow_documented_geometry_formula() -> Result<(), Box<dyn std::error::Error>> {
    let state_1x = medium_planted_at_scale(1.0);
    let expected_watts_1x =
        recommended_heater_max_watts(&state_1x.geometry, &state_1x.process_params);

    assert!(
        (state_1x.hardware.heater.max_watts - expected_watts_1x).abs() < 0.01,
        "heater should follow the documented geometry-aware formula: expected {expected_watts_1x:.1}W, got {:.1}W",
        state_1x.hardware.heater.max_watts
    );

    // AC: 100L stays near the 0.75 W/L midpoint, while the 10L nano can climb
    // toward the 1.0 W/L cap because its exposed-area heat loss per liter is higher.
    let scale_100l = (100.0_f64 / 57.6).cbrt();
    let state_100l = medium_planted_at_scale(scale_100l);
    assert!(
        (state_100l.hardware.heater.max_watts - 75.0).abs() < 5.0,
        "100L heater should stay near ~75W, got {:.1}W",
        state_100l.hardware.heater.max_watts
    );

    let state_nano = nano_at_scale(1.0);
    assert!(
        state_nano.hardware.heater.max_watts >= 8.0 && state_nano.hardware.heater.max_watts <= 10.8,
        "10.8L heater should remain in the conservative 0.75-1.0 W/L band, got {:.1}W",
        state_nano.hardware.heater.max_watts
    );

    Ok(())
}

#[test]
fn heater_watts_increase_for_high_exposure_same_volume_geometry() {
    let process_params = ProcessParams::default();
    let tall_geometry = TankGeometry {
        length_cm: 20.0,
        width_cm: 20.0,
        height_cm: 110.0,
        fill_height_cm: 100.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let shallow_geometry = TankGeometry {
        length_cm: 100.0,
        width_cm: 40.0,
        height_cm: 15.0,
        fill_height_cm: 10.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };

    assert!(
        (tall_geometry.gross_water_volume_l() - shallow_geometry.gross_water_volume_l()).abs()
            < 0.01,
        "test geometries should have the same volume"
    );

    let tall_watts = recommended_heater_max_watts(&tall_geometry, &process_params);
    let shallow_watts = recommended_heater_max_watts(&shallow_geometry, &process_params);

    assert!(
        shallow_watts > tall_watts,
        "higher exposed-area geometry should need more heater headroom: tall={tall_watts:.1}W, shallow={shallow_watts:.1}W"
    );
    assert!(
        shallow_watts <= shallow_geometry.gross_water_volume_l() * HEATER_MAX_WATTS_PER_LITER,
        "heater should remain capped by the 1.0 W/L upper guideline"
    );
    assert!(
        shallow_watts > shallow_geometry.gross_water_volume_l() * 0.75,
        "high-exposure geometry should rise above the plain 0.75 W/L midpoint"
    );
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
    // auto_stock_shrimp = true, no explicit count → auto-stock in the
    // conservative 0.15-0.25 adults/L band using actual filled water volume
    // after substrate displacement.
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

    let volume_l = state.water_volume_l();
    let expected_density = recommended_auto_stock_adults_per_liter(volume_l);
    let expected_count = (volume_l * expected_density).round() as u32;

    assert_eq!(
        state.animal.adult.count, expected_count,
        "auto-stocked count should use the documented density ramp: expected {expected_count} for {volume_l:.1}L"
    );
    assert!(
        expected_density >= AUTO_STOCK_MIN_ADULTS_PER_LITER
            && expected_density <= AUTO_STOCK_MAX_ADULTS_PER_LITER,
        "auto-stock density should stay in the conservative range, got {expected_density:.3}"
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

    let volume_2x = state_2x.water_volume_l();
    let expected_2x =
        (volume_2x * recommended_auto_stock_adults_per_liter(volume_2x)).round() as u32;

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

    let scale_100l = (100.0_f64 / 57.6).cbrt();
    let state_100l = seeded_state_with_full_overrides(
        SimSeed(303),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale: scale_100l,
                fill_ratio: 1.0,
            },
            auto_stock_shrimp: true,
            ..StartupOverrides::default()
        },
    )?;
    assert!(
        (15..=25).contains(&state_100l.animal.adult.count),
        "100L-style setup should auto-stock in the conservative 15-25 adult range, got {}",
        state_100l.animal.adult.count
    );

    let state_10l = seeded_state_with_full_overrides(
        SimSeed(304),
        "nano_cycle",
        StartupOverrides {
            auto_stock_shrimp: true,
            ..StartupOverrides::default()
        },
    )?;
    assert!(
        (1..=3).contains(&state_10l.animal.adult.count),
        "10L nano should auto-stock in the conservative 1-3 adult range, got {}",
        state_10l.animal.adult.count
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

// ---------------------------------------------------------------------------
// Combined 100L acceptance check (filter + heater + plant biomass)
// ---------------------------------------------------------------------------

/// AC: a 100L tank auto-scales to filter_flow ~1000 lph, heater ~75W, and
/// plant_biomass ~15g (proportional to footprint). Verifies all three together.
#[test]
fn combined_100l_scaling_produces_expected_hardware_and_biomass(
) -> Result<(), Box<dyn std::error::Error>> {
    // Scale medium_planted (57.6L base, 2 guilds) to ~100L
    let scale = (100.0_f64 / 57.6).cbrt();
    let state = medium_planted_at_scale(scale);

    let volume_l = state.geometry.gross_water_volume_l();
    assert!(
        (volume_l - 100.0).abs() < 1.0,
        "volume should be ~100L, got {volume_l:.1}L"
    );

    // Filter flow ≈ 1000 lph
    assert!(
        (state.hardware.filter.flow_lph - 1000.0).abs() < 15.0,
        "filter flow should be ~1000 lph, got {:.1}",
        state.hardware.filter.flow_lph
    );

    // Heater ≈ 75W
    assert!(
        (state.hardware.heater.max_watts - 75.0).abs() < 5.0,
        "heater should be ~75W, got {:.1}W",
        state.hardware.heater.max_watts
    );

    // Plant biomass ≈ 15g total (2 guilds × footprint-scaled per-guild)
    let footprint = state.geometry.footprint_area_cm2();
    let expected_per_guild = footprint * PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT / 1000.0;
    let expected_total = expected_per_guild * state.plant_guilds.len() as f64;
    let actual_total: f64 = state.plant_guilds.iter().map(|g| g.biomass_g).sum();

    assert!(
        (actual_total - expected_total).abs() < 0.5,
        "total plant biomass should be ~{expected_total:.1}g, got {actual_total:.1}g"
    );
    assert!(
        (actual_total - 15.0).abs() < 3.0,
        "total plant biomass should be ~15g for 100L, got {actual_total:.1}g"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Light intensity independence
// ---------------------------------------------------------------------------

/// Reviewer AC: light intensity (PAR at water surface) is a property of the
/// light fixture, not the tank. Verify it doesn't scale with volume.
#[test]
fn light_intensity_independent_of_tank_size() -> Result<(), Box<dyn std::error::Error>> {
    let state_1x = medium_planted_at_scale(1.0);
    let state_2x = medium_planted_at_scale(2.0);
    let state_nano = nano_at_scale(1.0);

    // Intensity index should be identical across all sizes
    assert!(
        (state_1x.hardware.light.intensity_index - state_2x.hardware.light.intensity_index).abs()
            < f64::EPSILON,
        "light intensity should not change with scale: 1×={:.2}, 2×={:.2}",
        state_1x.hardware.light.intensity_index,
        state_2x.hardware.light.intensity_index
    );
    assert!(
        (state_1x.hardware.light.intensity_index - state_nano.hardware.light.intensity_index).abs()
            < f64::EPSILON,
        "light intensity should not change between scenarios: medium={:.2}, nano={:.2}",
        state_1x.hardware.light.intensity_index,
        state_nano.hardware.light.intensity_index
    );

    // Photoperiod should also be a fixture property, not tank-scaled
    assert!(
        (state_1x.hardware.light.photoperiod_hours - state_2x.hardware.light.photoperiod_hours)
            .abs()
            < f64::EPSILON,
        "photoperiod should not change with scale: 1×={:.1}h, 2×={:.1}h",
        state_1x.hardware.light.photoperiod_hours,
        state_2x.hardware.light.photoperiod_hours
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Substrate depth independence
// ---------------------------------------------------------------------------

/// Reviewer AC: default substrate depth (cm) is the same regardless of tank
/// footprint. Total substrate volume scales with footprint, but depth is fixed.
#[test]
fn substrate_depth_independent_of_tank_size() -> Result<(), Box<dyn std::error::Error>> {
    let state_1x = medium_planted_at_scale(1.0);
    let state_2x = medium_planted_at_scale(2.0);

    // Both should have the same substrate layers
    assert_eq!(
        state_1x.substrate_layers.len(),
        state_2x.substrate_layers.len(),
        "same scenario should have same number of substrate layers"
    );

    // Each layer's depth_cm should be identical regardless of scale
    for (layer_1x, layer_2x) in state_1x
        .substrate_layers
        .iter()
        .zip(state_2x.substrate_layers.iter())
    {
        assert!(
            (layer_1x.depth_cm - layer_2x.depth_cm).abs() < f64::EPSILON,
            "substrate depth should not scale: 1×={:.2}cm, 2×={:.2}cm",
            layer_1x.depth_cm,
            layer_2x.depth_cm
        );
    }

    // Verify footprint actually differs (sanity check)
    let fp_ratio = state_2x.geometry.footprint_area_cm2() / state_1x.geometry.footprint_area_cm2();
    assert!(
        (fp_ratio - 4.0).abs() < 0.01,
        "footprint should be 4× for 2× geometry: ratio={fp_ratio:.3}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Aeration effect via habitat registry
// ---------------------------------------------------------------------------

/// Reviewer AC: aeration effect scales with surface area and volume, not just
/// a fixed rate. The aeration *setting* (intensity) is fixture-level, but the
/// physical effect in the habitat registry should scale with geometry.
#[test]
fn aeration_effect_scales_via_habitat_registry() -> Result<(), Box<dyn std::error::Error>> {
    use tank_core::HabitatKind;

    let state_1x = seeded_state_with_full_overrides(
        SimSeed(400),
        "medium_planted",
        StartupOverrides {
            aeration_enabled: Some(true),
            ..StartupOverrides::default()
        },
    )?;
    let state_2x = seeded_state_with_full_overrides(
        SimSeed(400),
        "medium_planted",
        StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale: 2.0,
                fill_ratio: 1.0,
            },
            aeration_enabled: Some(true),
            ..StartupOverrides::default()
        },
    )?;

    // Aeration intensity setting should be the same (fixture property)
    assert!(
        (state_1x.hardware.aeration.intensity - state_2x.hardware.aeration.intensity).abs()
            < f64::EPSILON,
        "aeration intensity setting should not scale"
    );

    // But colonizable areas (which determine aeration's physical effect) should
    // scale with geometry. Substrate surface area scales with footprint (4× for 2×).
    let substrate_area_1x = state_1x
        .habitat_registry
        .iter()
        .filter(|h| h.kind == HabitatKind::SubstrateSurface)
        .map(|h| h.colonizable_area_cm2)
        .sum::<f64>();
    let substrate_area_2x = state_2x
        .habitat_registry
        .iter()
        .filter(|h| h.kind == HabitatKind::SubstrateSurface)
        .map(|h| h.colonizable_area_cm2)
        .sum::<f64>();

    let area_ratio = substrate_area_2x / substrate_area_1x;
    assert!(
        area_ratio > 3.0 && area_ratio < 5.0,
        "substrate colonizable area should scale ~4× with 2× geometry: ratio={area_ratio:.2}"
    );

    Ok(())
}

fn thermal_probe_state(geometry: TankGeometry, ambient_temp_c: f64) -> TankState {
    let mut state = TankState::new(SimSeed(401));
    let process_params = ProcessParams::default();
    state.geometry = geometry;
    state.water =
        tank_core::WaterState::default_for_volume_l(state.geometry.gross_water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = ambient_temp_c;
    state.process_params = process_params.clone();
    state.hardware.heater.enabled = false;
    state.hardware.heater.max_watts =
        recommended_heater_max_watts(&state.geometry, &process_params);
    state.refresh_habitat_registry();
    state
}

#[test]
fn exposed_geometry_cools_faster_in_cool_ambient_conditions(
) -> Result<(), Box<dyn std::error::Error>> {
    let tall_geometry = TankGeometry {
        length_cm: 20.0,
        width_cm: 20.0,
        height_cm: 110.0,
        fill_height_cm: 100.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let shallow_geometry = TankGeometry {
        length_cm: 100.0,
        width_cm: 40.0,
        height_cm: 15.0,
        fill_height_cm: 10.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };

    let mut tall_engine = Engine::from_parts(thermal_probe_state(tall_geometry, 18.0), vec![]);
    let mut shallow_engine =
        Engine::from_parts(thermal_probe_state(shallow_geometry, 18.0), vec![]);

    tall_engine.step_hours(6)?;
    shallow_engine.step_hours(6)?;

    let tall_temp = tall_engine.snapshot().water_temp_c;
    let shallow_temp = shallow_engine.snapshot().water_temp_c;

    assert!(
        tall_temp > shallow_temp,
        "same-volume shallow geometry should cool faster in a cool room: tall={tall_temp:.2}C, shallow={shallow_temp:.2}C"
    );

    let tall_watts = recommended_heater_max_watts(
        &tall_engine.full_state().geometry,
        &tall_engine.full_state().process_params,
    );
    let shallow_watts = recommended_heater_max_watts(
        &shallow_engine.full_state().geometry,
        &shallow_engine.full_state().process_params,
    );
    assert!(
        shallow_watts > tall_watts,
        "heater sizing should track the faster-cooling exposed geometry: tall={tall_watts:.1}W, shallow={shallow_watts:.1}W"
    );

    Ok(())
}
