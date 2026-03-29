//! End-to-end conservation scenario runner.
//!
//! Runs all 6 conservation scenarios in sequence and produces a formatted
//! summary report showing pass/fail status with N and C delta magnitudes.
//! Exits non-zero if any scenario fails.
//!
//! Each scenario replicates the conservation story from the corresponding
//! unit test in `conservation_regression.rs`, wrapped in error handling
//! to ensure all 6 scenarios run even if early ones fail.
//!
//! Run: `cargo test -p tank_harness --test e2e_conservation -- --nocapture`
//! Set `TANK_BUDGET_DEBUG=1` for per-tick budget summaries on stderr.

use tank_core::{
    budget_helpers::{step_and_inspect, Element},
    plant_carbon_mg, plant_nitrogen_mg, Engine, JsonLinesSink, PlantGuildState, PlayerAction,
    SimSeed, SimTracer, SimulationEngine, SourceWaterProfile, TankState, TraceSink, Verbosity,
    WaterState,
};

const TOL: f64 = 1e-6;

// ---------------------------------------------------------------------------
// Outcome type
// ---------------------------------------------------------------------------

struct ScenarioOutcome {
    n_delta_mg: f64,
    c_delta_mg: f64,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn close_gas_exchange(state: &mut TankState) {
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
}

fn disable_dic_shortcuts(state: &mut TankState) {
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
}

fn dump_trace(label: &str, engine: &Engine) {
    let Some(tracer) = engine.tracer() else {
        return;
    };
    let dir = std::env::temp_dir().join("tank_conservation_e2e_traces");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{label}.jsonl"));
    let mut buf = Vec::new();
    {
        let mut sink = JsonLinesSink::new(&mut buf);
        for tick in tracer.ticks() {
            let _ = sink.emit_tick(tick);
        }
    }
    let _ = std::fs::write(&path, &buf);
}

fn fail_scenario<T>(label: &str, engine: &Engine, message: String) -> Result<T, String> {
    dump_trace(label, engine);
    Err(message)
}

fn ensure<F>(label: &str, engine: &Engine, condition: bool, message: F) -> Result<(), String>
where
    F: FnOnce() -> String,
{
    if condition {
        Ok(())
    } else {
        fail_scenario(label, engine, message())
    }
}

fn ensure_close(
    label: &str,
    engine: &Engine,
    actual: f64,
    expected: f64,
    tolerance: f64,
    context: &str,
) -> Result<(), String> {
    ensure(
        label,
        engine,
        (actual - expected).abs() <= tolerance,
        || format!("{context}: expected {expected}, got {actual} (tolerance {tolerance})"),
    )
}

fn trace_has_pool_delta<F>(engine: &Engine, system: &str, pool: &str, predicate: F) -> bool
where
    F: Fn(f64) -> bool,
{
    let Some(tracer) = engine.tracer() else {
        return false;
    };
    tracer
        .ticks()
        .iter()
        .filter_map(|tick| tick.system(system))
        .any(|entry| predicate(entry.delta_for(pool)))
}

fn ensure_trace_delta<F>(
    label: &str,
    engine: &Engine,
    system: &str,
    pool: &str,
    predicate: F,
    description: &str,
) -> Result<(), String>
where
    F: Fn(f64) -> bool,
{
    ensure(
        label,
        engine,
        trace_has_pool_delta(engine, system, pool, predicate),
        || format!("missing trace evidence for {description}: expected {system} to move {pool}"),
    )
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn grazing_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_1));
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
    state.water.dissolved_organic_carbon_mg_c_total = 0.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.0;
    state.water.ammonia_total_mg_n_total = 1.0;
    state.water.nitrite_mg_n_total = 0.0;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.periphyton_biomass_g = 5.0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;
    state.animal.adult.count = 8;
    state.animal.adult.condition_index = 0.8;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.enabled = false;
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.reseed_stability_tracker();
    state
}

fn feeding_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_2));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 1.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.16;
    state.water.ammonia_total_mg_n_total = 2.0;
    state.water.nitrite_mg_n_total = 0.5;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.phosphate_mg_p_total = 2.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.periphyton_biomass_g = 2.0;
    state.algae.suspended_biomass_g = 0.1;
    state.microbe.decomposer_biomass_g = 0.15;
    state.microbe.ammonia_oxidizer_biomass_g = 0.1;
    state.microbe.nitrite_oxidizer_biomass_g = 0.08;
    state.microbe.comammox_biomass_g = 0.03;
    state.microfauna.population_index = 0.3;
    state.microfauna.grazing_pressure_index = 0.2;
    state.animal.adult.count = 5;
    state.animal.adult.condition_index = 0.8;
    state.detritus.particulate_organics_g_total += 0.5;
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 10.0;
    state.hardware.light.intensity_index = 0.7;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.reseed_stability_tracker();
    state
}

fn mortality_senescence_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_3));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 2.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.5;
    state.water.ammonia_total_mg_n_total = 2.0;
    state.water.nitrite_mg_n_total = 0.5;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    if !state.plant_guilds.is_empty() {
        state.plant_guilds[0].biomass_g = 5.0;
    }
    if state.plant_guilds.len() > 1 {
        state.plant_guilds[1].biomass_g = 4.0;
    }
    state.process_params.plant_respiration_fraction_per_day = 0.08;
    state.process_params.plant_senescence_fraction_per_day = 0.18;
    state.animal.adult.count = 8;
    state.animal.adult.condition_index = 0.0;
    state.animal.molt_stress_index = 1.0;
    state.process_params.shrimp_base_mortality_per_day = 0.3;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.microbe.decomposer_biomass_g = 0.15;
    state.microbe.ammonia_oxidizer_biomass_g = 0.1;
    state.microbe.nitrite_oxidizer_biomass_g = 0.08;
    state.microbe.comammox_biomass_g = 0.03;
    state.algae.periphyton_biomass_g = 1.0;
    state.algae.suspended_biomass_g = 0.4;
    state.process_params.algae_respiration_fraction_per_day = 0.25;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state.hardware.light.enabled = false;
    state.hardware.light.photoperiod_hours = 0.0;
    state.hardware.light.intensity_index = 0.0;
    state.reseed_stability_tracker();
    state
}

fn trim_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_4));
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state.hardware.light.enabled = false;
    state.plant_guilds = vec![PlantGuildState::default(), PlantGuildState::default()];
    state.plant_guilds[0].biomass_g = 2.0;
    state.plant_guilds[1].biomass_g = 3.5;
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;
    state.reseed_stability_tracker();
    state
}

fn water_change_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_6));
    state.environment.ambient_temp_c = state.water.temperature_c;
    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
    state.hardware.filter.enabled = false;
    state.hardware.filter.flow_lph = 0.0;
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;
    for layer in &mut state.substrate_layers {
        layer.nutrient_store_mg_n_total = 0.0;
        layer.nutrient_store_mg_p_total = 0.0;
    }
    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrite_mg_n_total = 3.0;
    state.water.nitrate_mg_n_total = 18.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 240.0;
    state.water.dissolved_organic_carbon_mg_c_total = 16.0;
    let mut source = SourceWaterProfile::zero();
    source.temperature_c = 24.0;
    source.ammonia_mg_n_per_l = 0.5;
    source.nitrate_mg_n_per_l = 5.0;
    source.don_mg_n_per_l = 0.5;
    source.dic_mg_c_per_l = 30.0;
    source.doc_mg_c_per_l = 4.0;
    source.alkalinity_meq_per_l = 2.5;
    state
        .source_water_catalog
        .insert("test_source".to_string(), source);
    state.reseed_stability_tracker();
    state
}

// ---------------------------------------------------------------------------
// Scenario runners — each returns Ok(outcome) or Err(message)
// ---------------------------------------------------------------------------

fn run_grazing() -> Result<ScenarioOutcome, String> {
    let state = grazing_fixture();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 48).map_err(|e| e.to_string())?;
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace("grazing", &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_feeding() -> Result<ScenarioOutcome, String> {
    let label = "feeding";
    let state = feeding_fixture();
    let initial_particulate = state.detritus.particulate_organics_g_total;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 120).map_err(|e| e.to_string())?;
    let after = engine.full_state();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    ensure(
        label,
        &engine,
        after.detritus.particulate_organics_g_total < initial_particulate,
        || {
            format!(
                "feed should leave particulate organics: before={initial_particulate}, after={}",
                after.detritus.particulate_organics_g_total
            )
        },
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "detritus.particulate_g",
        |delta| delta < 0.0,
        "feed leaching out of particulate organics",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "detritus.feed_residue_g",
        |delta| delta > 0.0,
        "feed dissolution into dissolved residue",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "water.dic_mg_c",
        |delta| delta > 0.0,
        "decomposer mineralization producing DIC",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "water.nitrate_mg_n",
        |delta| delta > 0.0,
        "nitrification producing nitrate",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_microfauna",
        "algae.periphyton_g",
        |delta| delta < 0.0,
        "microfauna grazing periphyton",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_mortality_senescence() -> Result<ScenarioOutcome, String> {
    let label = "mortality_senescence";
    let state = mortality_senescence_fixture();
    let initial_shrimp = state.animal.adult.count;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 120).map_err(|e| e.to_string())?;
    let after = engine.full_state();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    ensure(
        label,
        &engine,
        after.animal.adult.count < initial_shrimp,
        || {
            format!(
                "some shrimp should have died: before={initial_shrimp}, after={}",
                after.animal.adult.count
            )
        },
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_plants",
        "plants.biomass_g",
        |delta| delta < 0.0,
        "plant senescence reducing plant biomass",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_plants",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "plant senescence routing biomass to fine detritus",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_algae",
        "algae.suspended_g",
        |delta| delta < 0.0,
        "algae senescence reducing suspended algae biomass",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_algae",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "algae senescence routing biomass to fine detritus",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_trim_and_remove() -> Result<ScenarioOutcome, String> {
    let label = "trim_and_remove";
    let state = trim_fixture();
    let fraction = 0.25;
    let trimmed_biomass_g: f64 = state
        .plant_guilds
        .iter()
        .map(|p| p.biomass_g * fraction)
        .sum();
    let expected_n_export = plant_nitrogen_mg(trimmed_biomass_g);
    let expected_c_export =
        plant_carbon_mg(trimmed_biomass_g, state.process_params.feed_n_to_c_ratio);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::TrimPlantsAndRemove { fraction })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let actual_export = initial_total_n - final_total_n;
    ensure_close(
        label,
        &engine,
        actual_export,
        expected_n_export,
        TOL,
        "trim-and-remove nitrogen export",
    )?;
    ensure_close(
        label,
        &engine,
        initial_total_c - final_total_c,
        expected_c_export,
        TOL,
        "trim-and-remove carbon export",
    )?;
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_trim_and_leave() -> Result<ScenarioOutcome, String> {
    let label = "trim_and_leave";
    let state = trim_fixture();
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let fraction = 0.25;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::TrimPlantsAndLeaveCuttings { fraction })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    ensure_close(
        label,
        &engine,
        final_total_n,
        initial_total_n,
        TOL,
        "trim-and-leave total nitrogen",
    )?;
    ensure_close(
        label,
        &engine,
        final_total_c,
        initial_total_c,
        TOL,
        "trim-and-leave total carbon",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_water_change() -> Result<ScenarioOutcome, String> {
    let label = "water_change";
    let state = water_change_fixture();
    let percent = 25.0;
    let fraction = percent / 100.0;
    let vol = state.water_volume_l();
    let exchanged_l = vol * fraction;
    let expected_n_export = fraction
        * (state.water.ammonia_total_mg_n_total
            + state.water.nitrite_mg_n_total
            + state.water.nitrate_mg_n_total
            + state.water.dissolved_organic_nitrogen_mg_n_total);
    let source = state.source_water_catalog.get("test_source").unwrap();
    let expected_n_import = exchanged_l
        * (source.ammonia_mg_n_per_l + source.nitrate_mg_n_per_l + source.don_mg_n_per_l);
    let expected_c_export = fraction
        * (state.water.dissolved_inorganic_carbon_mg_c_total
            + state.water.dissolved_organic_carbon_mg_c_total);
    let expected_c_import = exchanged_l * (source.dic_mg_c_per_l + source.doc_mg_c_per_l);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::WaterChangePercent {
            percent,
            source_profile_id: "test_source".to_string(),
        })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let actual_change = final_total_n - initial_total_n;
    let expected_change = expected_n_import - expected_n_export;
    ensure_close(
        label,
        &engine,
        actual_change,
        expected_change,
        TOL,
        "water-change total nitrogen delta",
    )?;
    ensure_close(
        label,
        &engine,
        final_total_c - initial_total_c,
        expected_c_import - expected_c_export,
        TOL,
        "water-change total carbon delta",
    )?;
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

// ---------------------------------------------------------------------------
// E2E runner
// ---------------------------------------------------------------------------

/// Runs all 6 conservation scenarios, produces a summary report, exits
/// non-zero on any failure.
#[test]
fn conservation_e2e_all_scenarios() {
    type ScenarioFn = fn() -> Result<ScenarioOutcome, String>;
    let scenarios: &[(&str, ScenarioFn)] = &[
        ("grazing", run_grazing),
        ("feeding", run_feeding),
        ("mortality_senescence", run_mortality_senescence),
        ("trim_and_remove", run_trim_and_remove),
        ("trim_and_leave", run_trim_and_leave),
        ("water_change", run_water_change),
    ];

    let mut passed = 0usize;
    let mut failed_count = 0usize;
    let mut rows: Vec<(&str, &str, f64, f64, String)> = Vec::new();

    for &(name, run_fn) in scenarios {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_fn)) {
            Ok(Ok(outcome)) => {
                rows.push((
                    name,
                    "PASS",
                    outcome.n_delta_mg,
                    outcome.c_delta_mg,
                    String::new(),
                ));
                passed += 1;
            }
            Ok(Err(msg)) => {
                rows.push((name, "FAIL", f64::NAN, f64::NAN, msg));
                failed_count += 1;
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown panic".into());
                // Truncate long panic messages for the summary
                let short = if msg.len() > 200 {
                    format!("{}...", &msg[..200])
                } else {
                    msg
                };
                rows.push((name, "PANIC", f64::NAN, f64::NAN, short));
                failed_count += 1;
            }
        }
    }

    // Print summary report
    eprintln!();
    eprintln!("{:=<76}", "");
    eprintln!(
        "  Conservation E2E Summary -- {} passed, {} failed",
        passed, failed_count
    );
    eprintln!("{:=<76}", "");
    eprintln!(
        "{:<28} {:>6} {:>16} {:>16}",
        "Scenario", "Result", "dN (mg)", "dC (mg)"
    );
    eprintln!("{:-<76}", "");
    for (name, result, n, c, msg) in &rows {
        if n.is_nan() {
            eprintln!("{:<28} {:>6} {:>16} {:>16}", name, result, "---", "---");
            if !msg.is_empty() {
                // Print first line of error only for readability
                let first_line = msg.lines().next().unwrap_or(msg);
                eprintln!("  {first_line}");
            }
        } else {
            eprintln!("{:<28} {:>6} {:>+16.9} {:>+16.9}", name, result, n, c);
        }
    }
    eprintln!("{:=<76}", "");
    eprintln!();

    assert_eq!(
        failed_count, 0,
        "{failed_count} conservation scenario(s) failed"
    );
}
