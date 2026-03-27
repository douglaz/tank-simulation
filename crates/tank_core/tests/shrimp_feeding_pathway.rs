/// Tests for the shrimp ingestion → assimilation → excretion → feces → respiration loop.
///
/// These tests verify the consumer routing contract from docs/ROUTING.md:
///   ingested = feces + assimilated
///   assimilated = respired + excreted + retained
///   fecal_fraction = 1.0 - assimilation_efficiency
///   respiration_fraction + excretion_fraction + growth_fraction = 1.0
use tank_core::{Engine, ProcessParams, SimError, SimSeed, SimulationEngine, TankState, WaterState};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

/// Creates a minimal tank with shrimp and food, suited for feeding pathway tests.
/// Disables plants, algae growth, nitrifiers, and decomposers to isolate
/// the shrimp feeding loop from other mass flows.
fn feeding_test_state() -> TankState {
    let mut state = TankState::new(SimSeed(9999));
    // Reasonable tank size
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    // Provide ample periphyton and detritus as food
    state.algae.periphyton_biomass_g = 5.0;
    state.detritus.fine_detritus_g_total = 2.0;

    // Stock shrimp
    state.animal.adults_count = 10;
    state.animal.condition_index = 0.8;

    // Disable all other biological processes to isolate feeding
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;

    state.process_params = ProcessParams::default();
    // Zero out background BOD so only shrimp feeding O2 demand shows
    state.process_params.background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    // Zero out detritus dissolution so feeding is the only source of change
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;

    state.reseed_stability_tracker();
    state
}

#[test]
fn default_fraction_sums_are_valid() {
    let params = ProcessParams::default();
    // fecal_fraction = 1.0 - assimilation_efficiency (implicit)
    assert!(
        params.shrimp_assimilation_efficiency > 0.0
            && params.shrimp_assimilation_efficiency < 1.0,
        "assimilation efficiency must be in (0, 1)"
    );
    // respiration + excretion + growth = 1.0
    let sum = params.shrimp_respiration_fraction_of_assimilated
        + params.shrimp_excretion_fraction_of_assimilated
        + params.shrimp_growth_fraction_of_assimilated;
    assert_close(sum, 1.0, 1e-12);
}

#[test]
fn feeding_generates_tan_and_dic_and_feces() -> Result<(), SimError> {
    let state = feeding_test_state();
    let tan_before = state.water.ammonia_total_mg_n_total;
    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;
    let detritus_before = state.detritus.fine_detritus_g_total;

    let mut engine = Engine::from_parts(state, vec![]);
    // Run 24 hours (one full day cycle which includes shrimp feeding)
    engine.step_hours(24)?;

    let after = engine.full_state();
    // Feeding should generate TAN from excretion + respiration
    assert!(
        after.water.ammonia_total_mg_n_total > tan_before,
        "TAN should increase from shrimp excretion/respiration: before={tan_before}, after={}",
        after.water.ammonia_total_mg_n_total
    );
    // Feeding should generate DIC from respiration
    assert!(
        after.water.dissolved_inorganic_carbon_mg_c_total > dic_before,
        "DIC should increase from shrimp respiration: before={dic_before}, after={}",
        after.water.dissolved_inorganic_carbon_mg_c_total
    );
    // Feces should add to detritus (net increase depends on whether dissolution is off)
    // With dissolution off, fecal output should be visible
    assert!(
        after.detritus.fine_detritus_g_total != detritus_before,
        "fine detritus should change from fecal output"
    );
    // Reserve should accumulate from retained share
    assert!(
        after.animal.reserve_g > 0.0,
        "reserve should accumulate from feeding: got {}",
        after.animal.reserve_g
    );
    Ok(())
}

#[test]
fn feeding_consumes_dissolved_oxygen() -> Result<(), SimError> {
    let mut state = feeding_test_state();
    // Disable aeration to isolate O2 consumption
    state.hardware.aeration.enabled = false;
    let do_before = state.water.dissolved_oxygen_mg_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let after = engine.full_state();
    // The respiration pathway should have consumed O2
    assert!(
        after.water.dissolved_oxygen_mg_total < do_before,
        "DO should decrease from shrimp respiration: before={do_before}, after={}",
        after.water.dissolved_oxygen_mg_total
    );
    Ok(())
}

#[test]
fn closed_system_shrimp_feeding_conserves_nitrogen() -> Result<(), SimError> {
    let state = feeding_test_state();
    let n_before = state.total_nitrogen();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    // Run 7 days to accumulate feeding effects
    engine.step_hours(24 * 7)?;

    let n_after = engine.full_state().total_nitrogen();
    // Nitrogen must be conserved: consumed periphyton/detritus N should
    // reappear as TAN + fecal detritus N + reserve N (no external inputs/outputs).
    assert_close(n_after, n_before, 0.01);
    Ok(())
}

#[test]
fn closed_system_shrimp_feeding_conserves_carbon() -> Result<(), SimError> {
    let state = feeding_test_state();
    let c_before = state.total_carbon();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.step_hours(24 * 7)?;

    let c_after = engine.full_state().total_carbon();
    // Carbon must be conserved: consumed periphyton/detritus C should
    // reappear as DIC + DOC + fecal detritus C + reserve C.
    assert_close(c_after, c_before, 0.01);
    Ok(())
}

#[test]
fn shrimp_mortality_transfers_reserve_to_detritus() -> Result<(), SimError> {
    let mut state = feeding_test_state();
    // Give shrimp a starting reserve
    state.animal.reserve_g = 0.05;

    // Kill all shrimp by extreme stress
    state.water.ammonia_total_mg_n_total = 200.0; // lethal TAN
    state.water.dissolved_oxygen_mg_total = 0.1; // near-zero DO
    state.water.temperature_c = 38.0; // extreme heat
    // Maximize mortality rates
    state.process_params.shrimp_base_mortality_per_day = 0.3;
    state.process_params.shrimp_stress_mortality_scale = 0.5;

    // Snapshot AFTER setting stress conditions
    let n_before = state.total_nitrogen();
    let c_before = state.total_carbon();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.step_hours(24 * 30)?; // 30 days for complete die-off

    let after = engine.full_state();
    // All shrimp should be dead
    assert_eq!(
        after.animal.adults_count + after.animal.juveniles_count,
        0,
        "all shrimp should be dead"
    );
    // Reserve should be drained (all shrimp dead → proportional transfer emptied it)
    assert_close(after.animal.reserve_g, 0.0, 1e-9);
    // N and C should still be conserved
    let n_after = after.total_nitrogen();
    let c_after = after.total_carbon();
    assert_close(n_after, n_before, 0.01);
    assert_close(c_after, c_before, 0.01);
    Ok(())
}

#[test]
fn assimilated_plus_fecal_equals_consumed() {
    // Direct unit test of the routing fractions.
    let params = ProcessParams::default();
    let ae = params.shrimp_assimilation_efficiency;
    let fecal = 1.0 - ae;

    // For 1g of food consumed (in N+C elemental mg):
    let consumed_n = 100.0; // arbitrary mg
    let consumed_c = 625.0; // at 0.16 N:C ratio

    let fecal_n = consumed_n * fecal;
    let fecal_c = consumed_c * fecal;
    let assimilated_n = consumed_n * ae;
    let assimilated_c = consumed_c * ae;

    assert_close(fecal_n + assimilated_n, consumed_n, 1e-12);
    assert_close(fecal_c + assimilated_c, consumed_c, 1e-12);

    // Of the assimilated share:
    let resp_frac = params.shrimp_respiration_fraction_of_assimilated;
    let excr_frac = params.shrimp_excretion_fraction_of_assimilated;
    let growth_frac = params.shrimp_growth_fraction_of_assimilated;

    let respired_n = assimilated_n * resp_frac;
    let excreted_n = assimilated_n * excr_frac;
    let retained_n = assimilated_n * growth_frac;
    assert_close(respired_n + excreted_n + retained_n, assimilated_n, 1e-12);

    let respired_c = assimilated_c * resp_frac;
    let excreted_c = assimilated_c * excr_frac;
    let retained_c = assimilated_c * growth_frac;
    assert_close(respired_c + excreted_c + retained_c, assimilated_c, 1e-12);

    // Total routing:
    // N: fecal_n (→ detritus) + respired_n (→ TAN) + excreted_n (→ TAN) + retained_n (→ reserve)
    let total_n_routed = fecal_n + respired_n + excreted_n + retained_n;
    assert_close(total_n_routed, consumed_n, 1e-12);

    // C: fecal_c (→ detritus) + respired_c (→ DIC) + excreted_c (→ DOC) + retained_c (→ reserve)
    let total_c_routed = fecal_c + respired_c + excreted_c + retained_c;
    assert_close(total_c_routed, consumed_c, 1e-12);
}
