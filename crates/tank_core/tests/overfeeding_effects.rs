use tank_core::{systems::light::is_light_on, Engine, PlayerAction, SimSeed, SimulationEngine};

#[test]
fn overfeeding_effects() -> Result<(), tank_core::SimError> {
    let control_state = configured_state(SimSeed(8800));
    let overfed_state = configured_state(SimSeed(8800));

    let mut control = Engine::from_parts(control_state, vec![]);
    let mut overfed = Engine::from_parts(overfed_state, vec![]);

    let mut control_nightly_min_do = f64::INFINITY;
    let mut overfed_nightly_min_do = f64::INFINITY;

    for _day in 0..14 {
        control.apply_action(PlayerAction::Feed { grams: 0.18 })?;
        overfed.apply_action(PlayerAction::Feed { grams: 0.85 })?;

        for _hour in 0..24 {
            control.step_hours(1)?;
            overfed.step_hours(1)?;

            let control_snapshot = control.snapshot();
            let overfed_snapshot = overfed.snapshot();

            if !is_light_on(control_snapshot.hour, control_snapshot.photoperiod_hours) {
                control_nightly_min_do = control_nightly_min_do.min(control_snapshot.do_mg_l);
            }
            if !is_light_on(overfed_snapshot.hour, overfed_snapshot.photoperiod_hours) {
                overfed_nightly_min_do = overfed_nightly_min_do.min(overfed_snapshot.do_mg_l);
            }
        }
    }

    let control_detritus = total_detritus_g(control.full_state());
    let overfed_detritus = total_detritus_g(overfed.full_state());

    assert!(
        overfed_detritus > control_detritus,
        "overfed tank should finish with more detritus: overfed={overfed_detritus:.3}, control={control_detritus:.3}"
    );
    assert!(
        overfed_nightly_min_do < control_nightly_min_do,
        "overfed tank should have a lower nightly DO minimum: overfed={overfed_nightly_min_do:.3}, control={control_nightly_min_do:.3}"
    );
    assert!(
        overfed.snapshot().algae_nuisance_index > control.snapshot().algae_nuisance_index,
        "overfed tank should have higher algae nuisance: overfed={:.3}, control={:.3}",
        overfed.snapshot().algae_nuisance_index,
        control.snapshot().algae_nuisance_index
    );

    Ok(())
}

fn configured_state(seed: SimSeed) -> tank_core::TankState {
    let mut state =
        tank_scenarios::seeded_state(seed, "warm_room").expect("scenario should materialize");
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.05;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;
    state.process_params.reaeration_kla_base *= 0.4;
    state.process_params.aeration_kla_boost *= 0.25;
    state.environment.ambient_temp_c = 29.5;
    state.algae.suspended_biomass_g = 0.08;
    state.algae.periphyton_biomass_g = 0.6;
    for plant in &mut state.plant_guilds {
        plant.biomass_g *= 0.5;
    }
    state
}

fn total_detritus_g(state: &tank_core::TankState) -> f64 {
    state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total
}
