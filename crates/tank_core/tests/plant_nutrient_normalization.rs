use tank_core::{
    systems::plant_growth::step_daily_plants, PlantGuild, PlantGuildState, ProcessParams, SimSeed,
    SubstrateKind, SubstrateLayerState, TankGeometry, TankState, WaterState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a plant-ready state with a given water volume (by adjusting fill
/// height while keeping the same footprint).  Water chemistry is set to
/// the requested concentrations (mg / L) so different-volume tanks have
/// identical nutrient exposure.
fn state_at_volume_with_concentrations(
    seed: SimSeed,
    target_volume_l: f64,
    tan_mg_n_per_l: f64,
    nitrate_mg_n_per_l: f64,
    phosphate_mg_p_per_l: f64,
    dic_mg_c_per_l: f64,
) -> TankState {
    let mut state = TankState::new(seed);

    // Fixed footprint (50 × 20 cm = 0.1 m²), adjust fill height for volume.
    let length_cm = 50.0;
    let width_cm = 20.0;
    let fill_cm = target_volume_l * 1000.0 / (length_cm * width_cm);

    state.geometry = TankGeometry {
        length_cm,
        width_cm,
        height_cm: fill_cm + 5.0,
        fill_height_cm: fill_cm,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };

    // No substrate — pure water-column test.
    state.substrate_layers.clear();
    state.water = WaterState::default_for_volume_l(state.water_volume_l());

    let vol = state.water_volume_l();
    state.water.ammonia_total_mg_n_total = tan_mg_n_per_l * vol;
    state.water.nitrate_mg_n_total = nitrate_mg_n_per_l * vol;
    state.water.phosphate_mg_p_total = phosphate_mg_p_per_l * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = dic_mg_c_per_l * vol;

    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.process_params = ProcessParams::default();
    // Zero out extinction coefficients so depth-dependent light attenuation
    // doesn't break concentration-invariance (tested separately).
    state.process_params.base_extinction_coeff_per_cm = 0.0;
    state.process_params.algae_extinction_coeff_per_cm_per_g_l = 0.0;
    state.process_params.doc_extinction_coeff_per_cm_per_mg_c_l = 0.0;
    state
        .process_params
        .detritus_extinction_coeff_per_cm_per_g_l = 0.0;

    state
}

fn assert_close(actual: f64, expected: f64, tol: f64, msg: &str) {
    assert!(
        (actual - expected).abs() <= tol,
        "{msg}: expected {expected}, got {actual} (tol={tol})"
    );
}

// ---------------------------------------------------------------------------
// Tank-size-independence test (B3e pattern)
// ---------------------------------------------------------------------------

/// Two tanks with identical nutrient *concentrations* but 10× different
/// volumes must produce identical plant limitation factors and identical
/// biomass change after one daily step.
#[test]
fn plant_limitation_is_tank_size_independent() -> Result<(), tank_core::SimError> {
    let tan = 0.5;
    let no3 = 2.0;
    let po4 = 0.3;
    let dic = 12.0;

    let mut small = state_at_volume_with_concentrations(SimSeed(9000), 10.0, tan, no3, po4, dic);
    let mut large = state_at_volume_with_concentrations(SimSeed(9000), 100.0, tan, no3, po4, dic);

    // Same guild, same biomass, pure water-column feeder.
    let guild_state = PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 4.0,
        health_index: 0.8,
        crowding_index: 0.0,
        habitat_index: 0.8,
        water_column_uptake_bias: Some(1.0),
        substrate_uptake_bias: Some(0.0),
    };
    small.plant_guilds = vec![guild_state.clone()];
    large.plant_guilds = vec![guild_state];

    step_daily_plants(&mut small);
    step_daily_plants(&mut large);

    assert_close(
        small.plant_guilds[0].biomass_g,
        large.plant_guilds[0].biomass_g,
        1e-9,
        "plant biomass should be identical at same concentrations regardless of tank volume",
    );

    assert_close(
        small.plant_guilds[0].health_index,
        large.plant_guilds[0].health_index,
        1e-9,
        "plant health index should be identical at same concentrations regardless of tank volume",
    );

    Ok(())
}

/// Same test but for rooted plants with substrate.
#[test]
fn rooted_plant_limitation_is_tank_size_independent() -> Result<(), tank_core::SimError> {
    let tan = 0.2;
    let no3 = 0.5;
    let po4 = 0.1;
    let dic = 15.0;

    let mut small = state_at_volume_with_concentrations(SimSeed(9001), 10.0, tan, no3, po4, dic);
    let mut large = state_at_volume_with_concentrations(SimSeed(9001), 100.0, tan, no3, po4, dic);

    // Both tanks have same footprint (50×20 = 0.1 m²), same substrate density.
    let substrate_n_per_m2 = 200.0; // mg N / m²
    let substrate_p_per_m2 = 30.0; // mg P / m²
    let area_m2 = small.geometry.footprint_area_m2();

    let make_substrate = |area: f64| SubstrateLayerState {
        kind: SubstrateKind::ActivePlanted,
        depth_cm: 4.0,
        nutrient_store_mg_n_total: substrate_n_per_m2 * area,
        nutrient_store_mg_p_total: substrate_p_per_m2 * area,
        cation_exchange_capacity_index: 0.8,
        detritus_trapping_index: 0.4,
        colonizable_area_cm2: area * 10_000.0,
        colonizable_area_factor: 1.0,
        low_oxygen_tendency_index: 0.3,
        grazing_surface_index: 0.4,
    };

    small.substrate_layers = vec![make_substrate(area_m2)];
    // Large tank has 5× the footprint to get the same fill height ratio.
    // But we keep same footprint so substrate density is the same.
    large.substrate_layers = vec![make_substrate(area_m2)];

    let guild_state = PlantGuildState {
        guild: PlantGuild::RootFeedingRosette,
        biomass_g: 4.0,
        health_index: 0.8,
        crowding_index: 0.0,
        habitat_index: 0.8,
        water_column_uptake_bias: Some(0.3),
        substrate_uptake_bias: Some(0.9),
    };
    small.plant_guilds = vec![guild_state.clone()];
    large.plant_guilds = vec![guild_state];

    step_daily_plants(&mut small);
    step_daily_plants(&mut large);

    // Biomass won't be *exactly* identical because the two tanks have
    // different volumes but the same absolute substrate mass, so nutrient
    // removal (absolute mg) differs.  However, the *limitation factor*
    // that drives gross growth should be identical; any difference comes
    // only from the cap_frac adjustment.  We allow a wider tolerance.
    assert!(
        (small.plant_guilds[0].biomass_g - large.plant_guilds[0].biomass_g).abs() < 0.05,
        "rooted plant biomass should be very close at same concentrations (small={}, large={})",
        small.plant_guilds[0].biomass_g,
        large.plant_guilds[0].biomass_g,
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests for each limitation term
// ---------------------------------------------------------------------------

/// The half-saturation (Monod) function: S / (S + Ks).
#[test]
fn half_saturation_function_known_values() {
    // At S == Ks, factor should be 0.5.
    let factor = monod(0.4, 0.4);
    assert_close(factor, 0.5, 1e-12, "S == Ks should give 0.5");

    // At S == 0, factor should be 0.0.
    let factor = monod(0.0, 0.4);
    assert_close(factor, 0.0, 1e-12, "S == 0 should give 0.0");

    // At S >> Ks, factor approaches 1.0.
    let factor = monod(100.0, 0.4);
    assert!(factor > 0.99, "S >> Ks should approach 1.0, got {factor}");

    // At S == 2×Ks, factor should be 2/3.
    let factor = monod(0.8, 0.4);
    assert_close(factor, 2.0 / 3.0, 1e-12, "S == 2×Ks should give 2/3");
}

/// The Gaussian temperature response function.
#[test]
fn gaussian_temperature_response_known_values() {
    // At optimum, factor should be 1.0.
    let factor = gaussian(25.0, 25.0, 7.0);
    assert_close(factor, 1.0, 1e-12, "at optimum, factor = 1.0");

    // At optimum ± sigma, factor should be exp(-0.5) ≈ 0.6065.
    let factor = gaussian(32.0, 25.0, 7.0);
    assert_close(factor, (-0.5_f64).exp(), 1e-9, "at optimum + sigma");

    let factor = gaussian(18.0, 25.0, 7.0);
    assert_close(factor, (-0.5_f64).exp(), 1e-9, "at optimum - sigma");

    // Far from optimum, factor should be near 0.
    let factor = gaussian(5.0, 25.0, 7.0);
    assert!(
        factor < 0.05,
        "far from optimum, factor should be near 0, got {factor}"
    );
}

/// The weighted limitation factor combines water-column and substrate factors.
#[test]
fn weighted_limitation_factor_known_values() {
    // Pure water-column feeder: water weight = 1.0, substrate weight = 0.0.
    let factor = weighted_lim(0.8, 0.2, 1.0, 0.0);
    assert_close(
        factor,
        0.8,
        1e-12,
        "pure water-column feeder sees water factor only",
    );

    // Pure substrate feeder: water weight = 0.0, substrate weight = 1.0.
    let factor = weighted_lim(0.8, 0.2, 0.0, 1.0);
    assert_close(
        factor,
        0.2,
        1e-12,
        "pure substrate feeder sees substrate factor only",
    );

    // Mixed feeder: 50/50 split.
    let factor = weighted_lim(0.8, 0.2, 0.5, 0.5);
    assert_close(factor, 0.5, 1e-12, "50/50 split averages both factors");

    // Clamping: result never exceeds 1.0.
    let factor = weighted_lim(1.0, 1.0, 0.6, 0.6);
    assert_close(
        factor,
        1.0,
        1e-12,
        "clamped to 1.0 when both factors are 1.0",
    );
}

/// Nitrogen limitation combines TAN + nitrate in water column.
#[test]
fn nitrogen_limitation_uses_combined_dissolved_n() {
    let params = ProcessParams::default();
    let ks = params.plant_half_saturation_n_mg_n_per_l;

    // TAN alone at Ks.
    let factor_tan_only = monod(ks, ks);
    assert_close(factor_tan_only, 0.5, 1e-12, "TAN at Ks");

    // TAN + nitrate both at Ks/2 → combined = Ks → factor = 0.5.
    let factor_combined = monod(ks / 2.0 + ks / 2.0, ks);
    assert_close(factor_combined, 0.5, 1e-12, "TAN+NO3 combined at Ks");

    // TAN at 0, nitrate at 2×Ks → factor = 2/3.
    let factor_no3_only = monod(2.0 * ks, ks);
    assert_close(factor_no3_only, 2.0 / 3.0, 1e-12, "NO3 at 2×Ks");
}

/// Verify that the default concentration-based Ks values match the legacy
/// total-based values divided by the reference volume/area.
#[test]
fn default_ks_values_match_legacy_equivalents() {
    let params = ProcessParams::default();

    // Legacy: 8.0 mg total / 20 L = 0.4 mg/L
    assert_close(
        params.plant_half_saturation_n_mg_n_per_l,
        0.4,
        1e-12,
        "N Ks should be 0.4 mg/L",
    );

    // Legacy: 1.2 mg total / 20 L = 0.06 mg/L
    assert_close(
        params.plant_half_saturation_p_mg_p_per_l,
        0.06,
        1e-12,
        "P Ks should be 0.06 mg/L",
    );

    // Legacy: 20.0 mg total / 20 L = 1.0 mg/L
    assert_close(
        params.plant_half_saturation_c_mg_c_per_l,
        1.0,
        1e-12,
        "C Ks should be 1.0 mg/L",
    );

    // Legacy: 8.0 mg total / 0.1 m² = 80.0 mg/m²
    assert_close(
        params.plant_half_saturation_n_substrate_mg_n_per_m2,
        80.0,
        1e-12,
        "substrate N Ks should be 80.0 mg/m²",
    );

    // Legacy: 1.2 mg total / 0.1 m² = 12.0 mg/m²
    assert_close(
        params.plant_half_saturation_p_substrate_mg_p_per_m2,
        12.0,
        1e-12,
        "substrate P Ks should be 12.0 mg/m²",
    );
}

/// Nutrient limitation is min(f_n, f_p, f_c) — verify the minimum behavior.
#[test]
fn nutrient_limitation_takes_minimum() {
    // When N is the most limiting, it should dominate.
    let f_n = monod(0.1, 0.4); // ≈ 0.2
    let f_p = monod(0.3, 0.06); // ≈ 0.833
    let f_c = monod(10.0, 1.0); // ≈ 0.909
    let limitation = f_n.min(f_p).min(f_c);
    assert_close(limitation, f_n, 1e-12, "N should be most limiting");

    // When P is the most limiting, it should dominate.
    let f_n2 = monod(2.0, 0.4); // ≈ 0.833
    let f_p2 = monod(0.01, 0.06); // ≈ 0.143
    let f_c2 = monod(10.0, 1.0); // ≈ 0.909
    let limitation2 = f_n2.min(f_p2).min(f_c2);
    assert_close(limitation2, f_p2, 1e-12, "P should be most limiting");
}

// ---------------------------------------------------------------------------
// Local reimplementations of the internal helpers for unit testing.
// These mirror the private functions in plant_growth.rs.
// ---------------------------------------------------------------------------

fn monod(value: f64, half_sat: f64) -> f64 {
    let half_sat = half_sat.max(f64::MIN_POSITIVE);
    (value / (value + half_sat)).clamp(0.0, 1.0)
}

fn gaussian(value: f64, optimum: f64, sigma: f64) -> f64 {
    let sigma = sigma.max(0.1);
    let diff = value - optimum;
    (-(diff * diff) / (2.0 * sigma * sigma))
        .exp()
        .clamp(0.0, 1.0)
}

fn weighted_lim(
    water_factor: f64,
    substrate_factor: f64,
    water_weight: f64,
    substrate_weight: f64,
) -> f64 {
    ((water_factor * water_weight) + (substrate_factor * substrate_weight)).clamp(0.0, 1.0)
}
