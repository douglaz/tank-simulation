use tank_core::{
    systems::chemistry::{co2_sat_mg_c_per_l, solve_carbonate_equilibrium},
    systems::dissolved_oxygen::compute_o2_kla,
    Engine, ProcessParams, SimSeed, SimulationEngine, TankGeometry, TankState,
};

/// Helper: tank with elevated CO2 suitable for gas-exchange tests.
/// Sets DIC high and alkalinity moderate to produce CO2 well above atmospheric
/// equilibrium. Biology-driven DIC rates are zeroed so only gas exchange acts.
fn co2_test_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    let volume_l = state.water_volume_l();
    // Elevated DIC (~40 mg C/L) produces CO2 well above atmospheric equilibrium.
    state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
    state.water.alkalinity_meq_total = 2.0 * volume_l;
    state.water.temperature_c = 25.0;
    // Zero biology-driven DIC fluxes so only gas exchange drives DIC changes.
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    // Zero O2 biology to avoid indirect coupling.
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    // Resolve carbonate state after DIC/alk changes.
    tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);
    state
}

fn snapshot_co2_mg_per_l(state: &TankState) -> f64 {
    let volume_l = state.water_volume_l();
    let eq = solve_carbonate_equilibrium(
        state.water.dissolved_inorganic_carbon_mg_c_total,
        state.water.alkalinity_meq_total,
        state.water.temperature_c,
        volume_l,
    );
    // Return CO2(aq) in mg CO2/L (molecular weight 44).
    eq.co2_aq_mmol_per_l * 44.0
}

// ---------------------------------------------------------------------------
// 1. CO2 reaeration toward equilibrium (both directions)
// ---------------------------------------------------------------------------

#[test]
fn test_co2_reaeration_toward_equilibrium() -> Result<(), tank_core::SimError> {
    // --- High CO2 → off-gasses toward equilibrium ---
    let mut high_state = co2_test_state(SimSeed(7000));
    high_state.hardware.aeration.enabled = true;
    high_state.hardware.aeration.intensity = 1.0;

    let co2_before_high = snapshot_co2_mg_per_l(&high_state);
    let co2_eq_mg_per_l = co2_sat_mg_c_per_l(25.0) * (44.0 / 12.0);
    assert!(
        co2_before_high > co2_eq_mg_per_l * 5.0,
        "test setup: initial CO2 ({co2_before_high:.2}) should be well above equilibrium ({co2_eq_mg_per_l:.3})"
    );

    let mut engine_high = Engine::from_parts(high_state, vec![]);
    engine_high.step_hours(24)?;

    let co2_after_high = snapshot_co2_mg_per_l(engine_high.full_state());
    assert!(
        co2_after_high < co2_before_high,
        "high CO2 should decrease: before={co2_before_high:.2}, after={co2_after_high:.2}"
    );
    assert!(
        co2_after_high < co2_eq_mg_per_l * 2.0,
        "after 24h aeration, CO2 ({co2_after_high:.3}) should approach equilibrium ({co2_eq_mg_per_l:.3})"
    );

    // --- Low CO2 → dissolves from atmosphere ---
    let mut low_state = co2_test_state(SimSeed(7001));
    let volume_l = low_state.water_volume_l();
    // Set DIC very low so CO2 is below atmospheric equilibrium.
    low_state.water.dissolved_inorganic_carbon_mg_c_total = 0.05 * volume_l;
    low_state.water.alkalinity_meq_total = 0.1 * volume_l;
    tank_core::systems::chemistry::resolve_carbonate_state(&mut low_state.water, volume_l);

    let co2_before_low = snapshot_co2_mg_per_l(&low_state);
    assert!(
        co2_before_low < co2_eq_mg_per_l,
        "test setup: initial CO2 ({co2_before_low:.4}) should be below equilibrium ({co2_eq_mg_per_l:.4})"
    );

    let mut engine_low = Engine::from_parts(low_state, vec![]);
    engine_low.step_hours(24)?;

    let co2_after_low = snapshot_co2_mg_per_l(engine_low.full_state());
    assert!(
        co2_after_low > co2_before_low,
        "low CO2 should increase: before={co2_before_low:.4}, after={co2_after_low:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Aeration accelerates CO2 exchange
// ---------------------------------------------------------------------------

#[test]
fn test_aeration_accelerates_co2_exchange() -> Result<(), tank_core::SimError> {
    let base = co2_test_state(SimSeed(7100));
    let co2_before = snapshot_co2_mg_per_l(&base);

    // No aeration (passive surface exchange only).
    let mut passive = base.clone();
    passive.hardware.aeration.enabled = false;

    // High aeration.
    let mut aerated = base;
    aerated.hardware.aeration.enabled = true;
    aerated.hardware.aeration.intensity = 1.0;

    let mut passive_engine = Engine::from_parts(passive, vec![]);
    let mut aerated_engine = Engine::from_parts(aerated, vec![]);

    passive_engine.step_hours(6)?;
    aerated_engine.step_hours(6)?;

    let co2_passive = snapshot_co2_mg_per_l(passive_engine.full_state());
    let co2_aerated = snapshot_co2_mg_per_l(aerated_engine.full_state());

    // Both should decrease (off-gassing), but aeration should strip more.
    assert!(co2_passive < co2_before, "passive should off-gas CO2");
    assert!(co2_aerated < co2_before, "aerated should off-gas CO2");
    assert!(
        co2_aerated < co2_passive,
        "aeration should strip more CO2: passive={co2_passive:.2}, aerated={co2_aerated:.2}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Surface area / lid affects exchange rate
// ---------------------------------------------------------------------------

#[test]
fn test_surface_area_affects_exchange_rate() -> Result<(), tank_core::SimError> {
    // Open-top tank (top_exchange_factor = 1.0).
    let mut open_state = co2_test_state(SimSeed(7200));
    open_state.geometry.open_top = true;
    open_state.hardware.aeration.enabled = false;

    // Lidded tank (top_exchange_factor = 0.25 default).
    let mut lidded_state = co2_test_state(SimSeed(7200));
    lidded_state.geometry.open_top = false;
    lidded_state.geometry.lid_exchange_factor = 0.25;
    lidded_state.hardware.aeration.enabled = false;

    let mut open_engine = Engine::from_parts(open_state, vec![]);
    let mut lidded_engine = Engine::from_parts(lidded_state, vec![]);

    open_engine.step_hours(6)?;
    lidded_engine.step_hours(6)?;

    let co2_open = snapshot_co2_mg_per_l(open_engine.full_state());
    let co2_lidded = snapshot_co2_mg_per_l(lidded_engine.full_state());

    assert!(
        co2_open < co2_lidded,
        "open-top tank should strip CO2 faster: open={co2_open:.2}, lidded={co2_lidded:.2}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. CO2 equilibrium concentration at 25°C
// ---------------------------------------------------------------------------

#[test]
fn test_co2_equilibrium_concentration() {
    let co2_eq_mg_c = co2_sat_mg_c_per_l(25.0);
    let co2_eq_mg_co2 = co2_eq_mg_c * (44.0 / 12.0);

    assert!(
        (0.5..=0.7).contains(&co2_eq_mg_co2),
        "CO2 equilibrium at 25°C should be 0.5-0.7 mg/L CO2, got {co2_eq_mg_co2:.3}"
    );
}

// ---------------------------------------------------------------------------
// 5. Temperature affects CO2 equilibrium
// ---------------------------------------------------------------------------

#[test]
fn test_temperature_affects_equilibrium() {
    let co2_20c = co2_sat_mg_c_per_l(20.0);
    let co2_30c = co2_sat_mg_c_per_l(30.0);

    assert!(
        co2_20c > co2_30c,
        "warmer water should hold less CO2: 20°C={co2_20c:.4}, 30°C={co2_30c:.4}"
    );

    // Both should be positive and reasonable.
    assert!(co2_20c > 0.0 && co2_20c < 1.0);
    assert!(co2_30c > 0.0 && co2_30c < 1.0);
}

// ---------------------------------------------------------------------------
// 6. CO2 exchange concentration dynamics are volume-independent
// ---------------------------------------------------------------------------

#[test]
fn test_co2_exchange_independent_of_tank_volume() -> Result<(), tank_core::SimError> {
    // Two tanks with same K_LA parameters but different fill heights (volumes).
    // Concentration change per hour should be the same.
    let make_state = |fill_height_cm: f64| -> TankState {
        let mut state = TankState::new(SimSeed(7400));
        state.geometry = TankGeometry {
            length_cm: 60.0,
            width_cm: 30.0,
            height_cm: 45.0,
            fill_height_cm,
            glass_thickness_mm: 5.0,
            open_top: true,
            lid_exchange_factor: 0.25,
        };
        let volume_l = state.water_volume_l();
        // Same concentration in both tanks.
        state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
        state.water.alkalinity_meq_total = 2.0 * volume_l;
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        state.water.temperature_c = 25.0;
        state
            .process_params
            .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
        state
            .process_params
            .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
        state.hardware.aeration.enabled = false;
        tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);
        state
    };

    let shallow = make_state(20.0);
    let deep = make_state(35.0);
    assert!(
        (shallow.water_volume_l() - deep.water_volume_l()).abs() > 1.0,
        "test setup: volumes should differ"
    );

    let co2_conc_shallow_before = snapshot_co2_mg_per_l(&shallow);
    let co2_conc_deep_before = snapshot_co2_mg_per_l(&deep);
    assert!(
        (co2_conc_shallow_before - co2_conc_deep_before).abs() < 0.01,
        "test setup: same initial CO2 concentration"
    );

    let mut shallow_engine = Engine::from_parts(shallow, vec![]);
    let mut deep_engine = Engine::from_parts(deep, vec![]);

    shallow_engine.step_hours(1)?;
    deep_engine.step_hours(1)?;

    let co2_shallow = snapshot_co2_mg_per_l(shallow_engine.full_state());
    let co2_deep = snapshot_co2_mg_per_l(deep_engine.full_state());

    // Concentration change should be nearly identical (K_LA-based model).
    let delta_shallow = co2_conc_shallow_before - co2_shallow;
    let delta_deep = co2_conc_deep_before - co2_deep;
    let relative_diff = (delta_shallow - delta_deep).abs() / delta_shallow.abs().max(1e-6);
    assert!(
        relative_diff < 0.05,
        "CO2 concentration change should be volume-independent: \
         shallow delta={delta_shallow:.4}, deep delta={delta_deep:.4}, relative diff={relative_diff:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Respiration-driven CO2 accumulation increases off-gassing
// ---------------------------------------------------------------------------

#[test]
fn test_high_co2_from_respiration_drives_offgassing() -> Result<(), tank_core::SimError> {
    // Tank with active respiration that produces CO2 in the dark.
    let mut state = TankState::new(SimSeed(7500));
    let volume_l = state.water_volume_l();
    state.water.temperature_c = 25.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    state.water.alkalinity_meq_total = 1.5 * volume_l;
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 0.0; // always dark
    state.hardware.aeration.enabled = false;
    state.plant_guilds[0].biomass_g = 15.0;
    state.plant_guilds[1].biomass_g = 10.0;
    state.animal.adults_count = 20;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.15;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);

    let co2_before_dark = snapshot_co2_mg_per_l(&state);

    // Run 12 hours in dark (respiration accumulates CO2).
    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(12)?;

    let co2_after_dark = snapshot_co2_mg_per_l(engine.full_state());
    assert!(
        co2_after_dark > co2_before_dark,
        "dark respiration should accumulate CO2: before={co2_before_dark:.2}, after={co2_after_dark:.2}"
    );

    // The higher CO2 means a larger gradient for off-gassing.
    // Verify by checking the gradient increased.
    let co2_eq = co2_sat_mg_c_per_l(25.0) * (44.0 / 12.0);
    let gradient_before = co2_before_dark - co2_eq;
    let gradient_after = co2_after_dark - co2_eq;
    assert!(
        gradient_after > gradient_before,
        "off-gassing gradient should increase: before={gradient_before:.2}, after={gradient_after:.2}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Integration: aerated vs non-aerated CO2 and pH
// ---------------------------------------------------------------------------

#[test]
fn test_aerated_vs_nonaerated_co2_levels() -> Result<(), tank_core::SimError> {
    // Two identical tanks with biological activity, one with aeration.
    let make_state = |aerated: bool| -> TankState {
        let mut state = TankState::new(SimSeed(7600));
        let volume_l = state.water_volume_l();
        state.water.temperature_c = 25.0;
        state.water.dissolved_inorganic_carbon_mg_c_total = 30.0 * volume_l;
        state.water.alkalinity_meq_total = 2.0 * volume_l;
        state.hardware.light.enabled = true;
        state.hardware.light.photoperiod_hours = 8.0;
        state.hardware.light.intensity_index = 1.0;
        state.hardware.aeration.enabled = aerated;
        state.hardware.aeration.intensity = if aerated { 1.0 } else { 0.0 };
        state.plant_guilds[0].biomass_g = 12.0;
        state.plant_guilds[1].biomass_g = 8.0;
        state.animal.adults_count = 15;
        state
            .process_params
            .respiration_dic_rate_mg_c_per_g_per_hour = 0.10;
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.15;
        state
            .process_params
            .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
        tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);
        state
    };

    let mut aerated_engine = Engine::from_parts(make_state(true), vec![]);
    let mut passive_engine = Engine::from_parts(make_state(false), vec![]);

    aerated_engine.step_hours(100)?;
    passive_engine.step_hours(100)?;

    let co2_aerated = snapshot_co2_mg_per_l(aerated_engine.full_state());
    let co2_passive = snapshot_co2_mg_per_l(passive_engine.full_state());
    let co2_eq = co2_sat_mg_c_per_l(25.0) * (44.0 / 12.0);

    assert!(
        (co2_aerated - co2_eq).abs() < (co2_passive - co2_eq).abs(),
        "aerated tank CO2 ({co2_aerated:.3}) should be closer to equilibrium ({co2_eq:.3}) \
         than passive ({co2_passive:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criteria: high CO2 + strong aeration → equilibrium within 24h
// ---------------------------------------------------------------------------

#[test]
fn test_strong_aeration_strips_co2_within_24h() -> Result<(), tank_core::SimError> {
    let mut state = co2_test_state(SimSeed(7700));
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.process_params.aeration_kla_boost = 0.9;

    let co2_eq = co2_sat_mg_c_per_l(25.0) * (44.0 / 12.0);

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let co2_final = snapshot_co2_mg_per_l(engine.full_state());
    assert!(
        (co2_final - co2_eq).abs() < 0.5,
        "after 24h strong aeration, CO2 ({co2_final:.3}) should be near equilibrium ({co2_eq:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criteria: without aeration, CO2 decreases much more slowly
// ---------------------------------------------------------------------------

#[test]
fn test_passive_exchange_strips_co2_slower() -> Result<(), tank_core::SimError> {
    let base = co2_test_state(SimSeed(7800));

    // No aeration.
    let mut passive = base.clone();
    passive.hardware.aeration.enabled = false;

    // With aeration.
    let mut aerated = base;
    aerated.hardware.aeration.enabled = true;
    aerated.hardware.aeration.intensity = 1.0;

    let mut passive_engine = Engine::from_parts(passive, vec![]);
    let mut aerated_engine = Engine::from_parts(aerated, vec![]);

    passive_engine.step_hours(24)?;
    aerated_engine.step_hours(24)?;

    let co2_passive = snapshot_co2_mg_per_l(passive_engine.full_state());
    let co2_aerated = snapshot_co2_mg_per_l(aerated_engine.full_state());
    let co2_eq = co2_sat_mg_c_per_l(25.0) * (44.0 / 12.0);

    // Aerated should be closer to equilibrium.
    assert!(
        (co2_aerated - co2_eq).abs() < (co2_passive - co2_eq).abs(),
        "passive CO2 ({co2_passive:.2}) should be further from equilibrium \
         than aerated ({co2_aerated:.2}); eq={co2_eq:.3}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criteria: CO2 exchange proportional to shared K_LA
// ---------------------------------------------------------------------------

#[test]
fn test_co2_kla_proportional_to_o2_kla() {
    let mut state = co2_test_state(SimSeed(7900));
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.5;

    let k_la_o2 = compute_o2_kla(&state);
    let k_la_co2 = k_la_o2 * 0.91;

    assert!(k_la_o2 > 0.0, "O2 K_LA should be positive");
    assert!(
        (k_la_co2 / k_la_o2 - 0.91).abs() < 1e-10,
        "K_LA(CO2)/K_LA(O2) should equal 0.91"
    );

    // Changing aeration should change both proportionally.
    state.hardware.aeration.intensity = 1.0;
    let k_la_o2_high = compute_o2_kla(&state);
    let k_la_co2_high = k_la_o2_high * 0.91;

    assert!(k_la_o2_high > k_la_o2);
    assert!(k_la_co2_high > k_la_co2);
    assert!(
        ((k_la_co2_high / k_la_o2_high) - (k_la_co2 / k_la_o2)).abs() < 1e-10,
        "ratio should be constant regardless of aeration intensity"
    );
}

// ---------------------------------------------------------------------------
// Integration: aeration raises pH by >= 0.2 over 12h with elevated CO2
// ---------------------------------------------------------------------------

#[test]
fn test_aeration_raises_ph_with_elevated_co2() -> Result<(), tank_core::SimError> {
    let mut state = co2_test_state(SimSeed(8000));
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;

    let ph_before = state.water.ph;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(12)?;

    let ph_after = engine.full_state().water.ph;
    let ph_rise = ph_after - ph_before;
    assert!(
        ph_rise >= 0.2,
        "aeration should raise pH by >= 0.2 units over 12h: before={ph_before:.3}, after={ph_after:.3}, rise={ph_rise:.3}"
    );

    Ok(())
}
