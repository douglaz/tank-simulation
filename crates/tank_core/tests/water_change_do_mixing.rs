use tank_core::{
    systems::temperature::do_sat_mg_l, Engine, PlayerAction, SimSeed, SimulationEngine,
    SourceWaterProfile, TankState,
};

#[test]
fn water_change_do_mixing() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(10_501));
    let volume_l = state.geometry.water_volume_l();

    state.water.temperature_c = 20.0;
    state.water.dissolved_oxygen_mg_total = 4.0 * volume_l;
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    state.hardware.light.enabled = false;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;

    let mut source = SourceWaterProfile::zero();
    source.temperature_c = 30.0;
    state
        .source_water_catalog
        .insert("warm_oxygenated".to_string(), source.clone());

    let expected_do_total = (state.water.dissolved_oxygen_mg_total * 0.5)
        + (do_sat_mg_l(source.temperature_c) * volume_l * 0.5);

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "warm_oxygenated".to_string(),
    })?;
    engine.step_hours(1)?;

    let actual = engine.full_state().water.dissolved_oxygen_mg_total;
    let relative_error = ((actual - expected_do_total) / expected_do_total).abs();
    assert!(
        relative_error <= 0.01,
        "50% water change should mix DO totals within 1%: expected={expected_do_total:.4}, actual={actual:.4}, rel_err={relative_error:.4}"
    );

    Ok(())
}
