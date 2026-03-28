/// Tests for the microfauna ingestion → assimilation → excretion → feces → respiration loop.
///
/// These tests verify the consumer routing contract from docs/ROUTING.md:
///   ingested = feces + assimilated
///   assimilated = respired + excreted + retained
///   fecal_fraction = 1.0 - assimilation_efficiency
///   respiration_fraction + excretion_fraction + growth_fraction = 1.0
use tank_core::{
    budget_helpers::{assert_c_conserved, assert_n_conserved, step_and_inspect},
    systems::microfauna::step_daily_microfauna,
    Engine, ProcessParams, SimError, SimSeed, TankState, WaterState,
};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

/// Creates a tank with active microfauna and food, but no shrimp, plants,
/// algae growth, nitrifiers, or decomposers — isolating the microfauna
/// feeding loop from other mass flows.
fn microfauna_test_state() -> TankState {
    let mut state = TankState::new(SimSeed(8_888));
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

    // Provide ample periphyton and detritus as food for microfauna
    state.algae.periphyton_biomass_g = 4.0;
    state.detritus.fine_detritus_g_total = 3.0;

    // Active microfauna population
    state.microfauna.population_index = 0.7;
    state.microfauna.grazing_pressure_index = 0.5;
    state.microfauna.reserve_g = 0.0;

    // No shrimp, no plants, no algae growth, no microbes
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;

    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
    state.hardware.filter.enabled = false;

    state.process_params = ProcessParams::default();
    // Closed-loop: no gas exchange
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    // Zero background BOD so only microfauna O2 demand shows
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    // Zero detritus dissolution so microfauna feeding is the only source of change
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;

    state.reseed_stability_tracker();
    state
}

// ── Fraction validity ──

#[test]
fn microfauna_default_fraction_sums_are_valid() {
    let params = ProcessParams::default();
    assert!(
        params.microfauna_assimilation_efficiency > 0.0
            && params.microfauna_assimilation_efficiency < 1.0,
        "assimilation efficiency must be in (0, 1)"
    );
    let sum = params.microfauna_respiration_fraction_of_assimilated
        + params.microfauna_excretion_fraction_of_assimilated
        + params.microfauna_growth_fraction_of_assimilated;
    assert_close(sum, 1.0, 1e-12);
}

// ── Conservation tests ──

#[test]
fn closed_system_microfauna_feeding_conserves_nitrogen() -> Result<(), SimError> {
    let mut engine = Engine::from_parts(microfauna_test_state(), vec![]);
    let result = step_and_inspect(&mut engine, 24 * 7)?;

    // Nitrogen must be conserved: consumed periphyton/detritus N should
    // reappear as TAN + fecal detritus N + reserve N.
    assert_n_conserved(&result.budget, 0.01);
    Ok(())
}

#[test]
fn closed_system_microfauna_feeding_conserves_carbon() -> Result<(), SimError> {
    let mut engine = Engine::from_parts(microfauna_test_state(), vec![]);
    let result = step_and_inspect(&mut engine, 24 * 7)?;

    // Carbon must be conserved: consumed periphyton/detritus C should
    // reappear as DIC + DOC + fecal detritus C + reserve C.
    assert_c_conserved(&result.budget, 0.01);
    Ok(())
}

// ── TAN increase and DO decrease tests ──

#[test]
fn microfauna_feeding_produces_tan_increase() {
    let mut state = microfauna_test_state();
    let tan_before = state.water.ammonia_total_mg_n_total;

    step_daily_microfauna(&mut state);

    assert!(
        state.water.ammonia_total_mg_n_total > tan_before,
        "TAN should increase from microfauna excretion/respiration: before={tan_before}, after={}",
        state.water.ammonia_total_mg_n_total
    );
}

#[test]
fn microfauna_feeding_produces_do_decrease() {
    let mut state = microfauna_test_state();
    let do_before = state.water.dissolved_oxygen_mg_total;

    step_daily_microfauna(&mut state);

    assert!(
        state.water.dissolved_oxygen_mg_total < do_before,
        "DO should decrease from microfauna respiration: before={do_before}, after={}",
        state.water.dissolved_oxygen_mg_total
    );
}

#[test]
fn microfauna_feeding_tan_and_do_proportional_to_ingestion() {
    // Higher population_index should produce proportionally more TAN and more DO draw.
    let mut state_low = microfauna_test_state();
    state_low.microfauna.population_index = 0.2;
    let mut state_high = state_low.clone();
    state_high.microfauna.population_index = 0.8;

    let tan_before = state_low.water.ammonia_total_mg_n_total;
    let do_before = state_low.water.dissolved_oxygen_mg_total;

    step_daily_microfauna(&mut state_low);
    step_daily_microfauna(&mut state_high);

    let tan_delta_low = state_low.water.ammonia_total_mg_n_total - tan_before;
    let tan_delta_high = state_high.water.ammonia_total_mg_n_total - tan_before;
    assert!(
        tan_delta_high > tan_delta_low,
        "higher population should produce more TAN: low={tan_delta_low}, high={tan_delta_high}"
    );

    let do_delta_low = do_before - state_low.water.dissolved_oxygen_mg_total;
    let do_delta_high = do_before - state_high.water.dissolved_oxygen_mg_total;
    assert!(
        do_delta_high > do_delta_low,
        "higher population should consume more DO: low={do_delta_low}, high={do_delta_high}"
    );
}

// ── Routing destination tests ──

#[test]
fn microfauna_feeding_generates_dic_and_feces() {
    let mut state = microfauna_test_state();
    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;
    let alk_before = state.water.alkalinity_meq_total;

    step_daily_microfauna(&mut state);

    assert!(
        state.water.dissolved_inorganic_carbon_mg_c_total > dic_before,
        "DIC should increase from microfauna respiration"
    );
    assert!(
        state.water.alkalinity_meq_total > alk_before,
        "alkalinity should increase alongside microfauna TAN release"
    );
    assert!(
        state.microfauna.reserve_g > 0.0,
        "microfauna reserve should accumulate retained share"
    );
}

#[test]
fn microfauna_single_step_conserves_n_and_c() {
    let mut state = microfauna_test_state();
    let n_before = state.total_nitrogen();
    let c_before = state.total_carbon();

    step_daily_microfauna(&mut state);

    assert_close(state.total_nitrogen(), n_before, 1e-6);
    assert_close(state.total_carbon(), c_before, 1e-6);
}
