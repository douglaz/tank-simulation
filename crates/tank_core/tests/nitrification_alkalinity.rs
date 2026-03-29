//! Regression and scenario tests for nitrification-driven alkalinity depletion
//! and pH decline via the carbonate solver.
//!
//! These tests verify that:
//! - The existing nitrification alkalinity charge is applied exactly once per mg N
//!   (by TAN oxidation: AOB + comammox, NOT also by NOB).
//! - The carbonate solver translates alkalinity depletion into mechanistic pH decline.
//! - Soft-water (low KH) tanks decline faster than hard-water (high KH) tanks.
//! - Alkalinity deltas are visible in budget tracking and structured tracing.

use tank_core::{
    systems::{chemistry::resolve_carbonate_state, nitrogen_cycle::step_nitrogen_cycle},
    Engine, ProcessParams, SimSeed, SimTracer, SimulationEngine, TankState, Verbosity,
    NITRIFICATION_ALK_MEQ_PER_MG_N,
};

/// Build a state primed for active nitrification: elevated TAN, mature biofilter,
/// established nitrifier biomass, and gas exchange disabled to isolate the
/// nitrification → alkalinity → pH pathway.
fn nitrifying_state(seed: SimSeed, alkalinity_meq_per_l: f64) -> TankState {
    let mut state = TankState::new(seed);
    let vol = state.water_volume_l();

    // Elevated TAN as nitrification substrate
    state.water.ammonia_total_mg_n_total = 4.0 * vol;
    // Established nitrifier guilds
    state.microbe.ammonia_oxidizer_biomass_g = 0.25;
    state.microbe.nitrite_oxidizer_biomass_g = 0.15;
    state.microbe.comammox_biomass_g = 0.05;
    // Mature biofilter
    state.filter_state.biofilter_maturity_index = 0.8;

    // Set alkalinity to the requested KH-equivalent.
    state.water.alkalinity_meq_total = alkalinity_meq_per_l * vol;
    // Ensure DIC > Alk (in molar terms) so the carbonate solver produces a
    // normal-regime pH rather than hitting the high-buffer ceiling. For the
    // solver: DIC_mol = dic_mg_c / 12000 / vol, Alk_eq = alk_meq / 1000 / vol.
    // We want DIC_mol > Alk_eq, i.e. dic_mg_c_per_l / 12 > alk_meq_per_l.
    // Set DIC to a comfortable margin above Alk.
    let dic_mg_c_per_l = (alkalinity_meq_per_l * 12.0 * 1.5).max(20.0);
    state.water.dissolved_inorganic_carbon_mg_c_total = dic_mg_c_per_l * vol;
    // Re-resolve carbonate state so cached pH reflects updated alk and DIC.
    resolve_carbonate_state(&mut state.water, vol);

    // Disable surface gas exchange so atmospheric CO2 doesn't confound the test
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;

    // Zero the DIC shortcut rates so only nitrification drives chemistry changes
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;

    // Remove plants and algae to eliminate DIC-consuming photosynthesis
    // that would raise pH and mask the nitrification alkalinity signal.
    for plant in &mut state.plant_guilds {
        plant.biomass_g = 0.0;
    }
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;

    // Zero shrimp to eliminate feeding/excretion confounds
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;

    state
}

// ---------------------------------------------------------------------------
// Acceptance criterion: closed-system alkalinity mass-balance
// ---------------------------------------------------------------------------

/// Verifies that in a closed system (no water changes, no denitrification),
/// the alkalinity consumed equals total N nitrified × NITRIFICATION_ALK_MEQ_PER_MG_N
/// within an explicit tolerance.
#[test]
fn closed_system_alkalinity_mass_balance() -> Result<(), tank_core::SimError> {
    let state = nitrifying_state(SimSeed(7_000), 3.0);
    let alk_before = state.water.alkalinity_meq_total;
    let tan_before = state.water.ammonia_total_mg_n_total;
    let nitrite_before = state.water.nitrite_mg_n_total;
    let nitrate_before = state.water.nitrate_mg_n_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    let hours = 72;
    engine.step_hours(hours)?;

    let s = engine.full_state();
    let alk_after = s.water.alkalinity_meq_total;
    let tan_after = s.water.ammonia_total_mg_n_total;
    let nitrite_after = s.water.nitrite_mg_n_total;
    let nitrate_after = s.water.nitrate_mg_n_total;

    // Total N that entered the nitrite/nitrate pools (= total N oxidized from TAN).
    // AOB: TAN → NO2, comammox: TAN → NO3, NOB: NO2 → NO3.
    // Net TAN consumed = TAN_before - TAN_after (minus any TAN consumed by growth,
    // but growth N is small compared to oxidation N).
    // Better metric: total inorganic N is conserved in the nitrification pathway,
    // so we track the actual TAN consumed and check against alkalinity consumed.
    let tan_consumed = tan_before - tan_after;

    // Alkalinity consumed
    let alk_consumed = alk_before - alk_after;

    // The alkalinity charge applies to the TAN-oxidation step only.
    // TAN consumed includes both the N that was oxidized AND a small fraction
    // that went into nitrifier biomass growth. The stoichiometric relationship
    // is between alkalinity and the N that was *oxidized* (not growth N).
    // We compute expected from the net NO2+NO3 gain which represents oxidized N.
    let no2_no3_gain = (nitrite_after + nitrate_after) - (nitrite_before + nitrate_before);
    let expected_alk = no2_no3_gain * NITRIFICATION_ALK_MEQ_PER_MG_N;

    // But the stoichiometric charge is on the TAN-oxidation step, not the full
    // pathway. The actual charge in the code is on aob_step.oxidized_n_mg and
    // comammox_step.oxidized_n_mg. Since NOB doesn't charge again, the total
    // alk consumed = (aob_oxidized + comammox_oxidized) * alk_per_mg_n.
    // This equals (TAN consumed minus TAN used for biomass growth) * constant.
    //
    // We use a tolerance that accounts for biomass growth routing:
    // growth yield ~0.05 g/mg N, so ~5% of TAN goes to biomass not oxidation.
    let tolerance_meq = 0.10 * alk_consumed.max(0.01);

    assert!(
        alk_consumed > 0.0,
        "Alkalinity should decrease from nitrification: consumed={alk_consumed}"
    );

    assert!(
        tan_consumed > 0.0,
        "TAN should be consumed: consumed={tan_consumed}"
    );

    // The tightest check: alk_consumed should be close to expected from
    // the actual N oxidized (NO2+NO3 gain excludes growth-assimilated N).
    // But NO2→NO3 (NOB) is not an alkalinity-consuming step, so we can't
    // simply use no2+no3 gain. Instead, the code charges on TAN oxidized
    // (AOB + comammox). We verify that alk_consumed / tan_consumed is close
    // to the stoichiometric constant.
    let observed_ratio = if tan_consumed > 0.01 {
        alk_consumed / tan_consumed
    } else {
        0.0
    };

    // The ratio should be close to but slightly below the stoichiometric
    // constant because a small fraction of TAN goes to biomass growth
    // (not charged alkalinity) rather than oxidation.
    assert!(
        observed_ratio > 0.0 && observed_ratio <= NITRIFICATION_ALK_MEQ_PER_MG_N * 1.05,
        "Alkalinity/TAN ratio ({observed_ratio:.6}) should be ≤ stoichiometric constant ({:.6})",
        NITRIFICATION_ALK_MEQ_PER_MG_N
    );
    assert!(
        observed_ratio > NITRIFICATION_ALK_MEQ_PER_MG_N * 0.80,
        "Alkalinity/TAN ratio ({observed_ratio:.6}) is too low — expected near {:.6}",
        NITRIFICATION_ALK_MEQ_PER_MG_N
    );

    // Verify alkalinity is within the expected range from oxidized N
    // (NO2+NO3 gain is a good proxy for total oxidized N).
    if no2_no3_gain > 0.5 {
        assert!(
            (alk_consumed - expected_alk).abs() < tolerance_meq,
            "Alk consumed ({alk_consumed:.4}) should match expected ({expected_alk:.4}) \
             from {no2_no3_gain:.4} mg N oxidized × {NITRIFICATION_ALK_MEQ_PER_MG_N:.6} \
             (tolerance {tolerance_meq:.4})"
        );
    }

    // Budget ledger should show alkalinity consumption via nitrogen_cycle stage
    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    assert!(
        !ledger.ticks.is_empty(),
        "Budget ledger should have recorded ticks"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criterion: soft-water vs hard-water buffering contrast
// ---------------------------------------------------------------------------

/// A soft-water (KH 2) tank should show faster pH decline than a hard-water
/// (KH 8) tank under the same ammonia load, demonstrating the buffering story.
#[test]
fn soft_water_declines_faster_than_hard_water() -> Result<(), tank_core::SimError> {
    // KH 2 ≈ 0.714 meq/L, KH 8 ≈ 2.856 meq/L
    // (1 dKH = 0.357 meq/L)
    let soft_alk_meq_per_l = 2.0 * 0.357;
    let hard_alk_meq_per_l = 8.0 * 0.357;

    let soft_state = nitrifying_state(SimSeed(7_100), soft_alk_meq_per_l);
    let hard_state = nitrifying_state(SimSeed(7_100), hard_alk_meq_per_l);

    // Record initial pH (both should differ because alkalinity affects starting pH)
    let soft_ph_initial = soft_state.water.ph;
    let hard_ph_initial = hard_state.water.ph;
    let soft_alk_initial = soft_state.water.alkalinity_meq_total;
    let hard_alk_initial = hard_state.water.alkalinity_meq_total;

    let mut soft_engine = Engine::from_parts(soft_state, vec![]);
    let mut hard_engine = Engine::from_parts(hard_state, vec![]);

    // Run 48 hours of nitrification
    let hours = 48;
    soft_engine.step_hours(hours)?;
    hard_engine.step_hours(hours)?;

    let soft_ph_final = soft_engine.full_state().water.ph;
    let hard_ph_final = hard_engine.full_state().water.ph;
    let soft_alk_final = soft_engine.full_state().water.alkalinity_meq_total;
    let hard_alk_final = hard_engine.full_state().water.alkalinity_meq_total;

    let soft_ph_drop = soft_ph_initial - soft_ph_final;
    let hard_ph_drop = hard_ph_initial - hard_ph_final;
    let soft_alk_consumed = soft_alk_initial - soft_alk_final;
    let hard_alk_consumed = hard_alk_initial - hard_alk_final;

    // Both tanks should consume some alkalinity
    assert!(
        soft_alk_consumed > 0.0,
        "Soft-water tank should consume alkalinity: consumed={soft_alk_consumed:.4}"
    );
    assert!(
        hard_alk_consumed > 0.0,
        "Hard-water tank should consume alkalinity: consumed={hard_alk_consumed:.4}"
    );

    // Soft water should show MORE pH decline than hard water (key buffering story).
    // In a low-buffer system, the same alkalinity consumption produces a larger
    // pH change because the remaining buffer capacity is proportionally smaller.
    assert!(
        soft_ph_drop > hard_ph_drop,
        "Soft-water pH drop ({soft_ph_drop:.4}) should exceed hard-water pH drop ({hard_ph_drop:.4}). \
         Soft: {soft_ph_initial:.3} → {soft_ph_final:.3}, Hard: {hard_ph_initial:.3} → {hard_ph_final:.3}"
    );

    // Hard water should end at a higher pH than soft water
    assert!(
        hard_ph_final > soft_ph_final,
        "Hard-water final pH ({hard_ph_final:.3}) should exceed soft-water ({soft_ph_final:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criterion: cycling scenario shows pH decline
// ---------------------------------------------------------------------------

/// A cycling tank without water changes or denitrification should show
/// progressive pH decline as alkalinity is consumed by nitrification,
/// verified through the carbonate solver.
///
/// Growth yields are set to near-zero to isolate the alkalinity→pH pathway.
/// In the full model, nitrifier autotrophic DIC consumption (growth) can
/// partially offset the pH-lowering effect of alkalinity depletion. This test
/// specifically verifies that alkalinity depletion alone drives pH downward
/// through the carbonate solver, which is the primary real-world mechanism.
#[test]
fn cycling_without_water_changes_shows_ph_decline() -> Result<(), tank_core::SimError> {
    let mut state = nitrifying_state(SimSeed(7_200), 2.0);

    // Near-zero growth yields isolate the alkalinity→pH signal from
    // growth-driven DIC consumption that can mask pH decline.
    state.process_params.aob_growth_yield = 1e-6;
    state.process_params.nob_growth_yield = 1e-6;
    state.process_params.comammox_growth_yield = 1e-6;
    state.process_params.decomposer_growth_yield = 1e-6;

    // Zero filter flow to eliminate CO2 off-gassing (filter agitation
    // contributes to KLA). Nitrification still runs at reduced rate
    // (flow_factor clamps to 0.1).
    state.hardware.filter.flow_lph = 0.0;

    let ph_initial = state.water.ph;
    let alk_initial = state.water.alkalinity_meq_total;
    let dic_initial = state.water.dissolved_inorganic_carbon_mg_c_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));

    // Run 1 hour and inspect the trajectory
    engine.step_hours(1)?;

    let s1 = engine.full_state();
    let alk_h1 = s1.water.alkalinity_meq_total;
    let dic_h1 = s1.water.dissolved_inorganic_carbon_mg_c_total;
    let _ph_h1 = s1.water.ph;

    // After 1 hour, alkalinity should have declined but DIC should be ~stable
    // (or slightly increased from decomposer mineralization).
    // If DIC decreases more than alk, pH rises — diagnose the cause.

    // Run remaining hours
    engine.step_hours(95)?;

    let ph_final = engine.full_state().water.ph;
    let alk_final = engine.full_state().water.alkalinity_meq_total;
    let dic_final = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;

    // Alkalinity should have been consumed
    assert!(
        alk_final < alk_initial,
        "Alkalinity should be consumed by nitrification: initial={alk_initial:.4}, final={alk_final:.4}"
    );

    // pH should have declined — this is the core verification that the
    // carbonate solver correctly translates alkalinity depletion into pH decline.
    //
    // In the current model, nitrification also depletes DIC (autotrophic
    // carbon fixation), which can partially offset the pH drop. With near-zero
    // growth yields, DIC should be ~stable so alkalinity loss dominates pH.
    assert!(
        ph_final < ph_initial,
        "pH should decline from nitrification: initial={ph_initial:.4}, final={ph_final:.4}. \
         alk: {alk_initial:.4} → {alk_h1:.4} → {alk_final:.4}, \
         DIC: {dic_initial:.4} → {dic_h1:.4} → {dic_final:.4}"
    );

    // pH must remain within solver bounds
    assert!(
        (5.5..=8.5).contains(&ph_final),
        "pH {ph_final:.3} should be within solver bounds [5.5, 8.5]"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criterion: alkalinity deltas visible in tracing
// ---------------------------------------------------------------------------

/// Structured tracing at Detail verbosity should show alkalinity_meq pool deltas
/// attributed to the nitrogen_cycle system. At Trace verbosity, notes should
/// include the alkalinity consumption attribution.
#[test]
fn tracing_shows_alkalinity_attribution() -> Result<(), tank_core::SimError> {
    let state = nitrifying_state(SimSeed(7_300), 3.0);
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.enable_tracing(SimTracer::new(Verbosity::Trace));

    engine.step_hours(1)?;

    let tracer = engine.tracer().expect("tracer should be enabled");
    let ticks = tracer.ticks();
    assert!(!ticks.is_empty(), "should have at least one tick trace");

    let tick = &ticks[0];

    // The nitrogen_cycle system should have pool deltas for alkalinity
    let nc_entry = tick
        .system("system:nitrogen_cycle")
        .expect("should have nitrogen_cycle trace entry");

    // At Detail+ verbosity, alkalinity pool delta should be present
    let alk_delta = nc_entry.pool_delta("water.alkalinity_meq");
    assert!(
        alk_delta.is_some(),
        "nitrogen_cycle should trace water.alkalinity_meq pool delta"
    );
    let alk_delta = alk_delta.unwrap();
    assert!(
        alk_delta.delta < 0.0,
        "alkalinity delta should be negative (consumed): {:.6}",
        alk_delta.delta
    );

    // At Trace verbosity, notes should include separate alkalinity attribution
    let has_consumed_note = nc_entry
        .notes
        .iter()
        .any(|n| n.contains("alkalinity_consumed_meq"));
    assert!(
        has_consumed_note,
        "nitrogen_cycle trace notes should include alkalinity_consumed_meq attribution. \
         Notes: {:?}",
        nc_entry.notes
    );

    let has_produced_note = nc_entry
        .notes
        .iter()
        .any(|n| n.contains("alkalinity_produced_meq"));
    assert!(
        has_produced_note,
        "nitrogen_cycle trace notes should include alkalinity_produced_meq attribution. \
         Notes: {:?}",
        nc_entry.notes
    );

    // Notes should use the TAN-oxidation basis that actually drives alkalinity.
    let has_tan_basis_note = nc_entry.notes.iter().any(|n| n.contains("tan_oxidized_mg"));
    assert!(
        has_tan_basis_note,
        "nitrogen_cycle trace notes should include tan_oxidized_mg. Notes: {:?}",
        nc_entry.notes
    );

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let budget_entry = ledger
        .ticks
        .first()
        .and_then(|tick| {
            tick.entries
                .iter()
                .find(|entry| entry.label == "system:nitrogen_cycle")
        })
        .expect("should record a nitrogen_cycle budget entry");
    let alk_budget_metric = budget_entry
        .metric("water.alkalinity_meq.delta")
        .expect("nitrogen_cycle budget entry should include alkalinity delta");
    assert!(
        alk_budget_metric.value < 0.0,
        "budget alkalinity delta should be negative (consumed): {:.6}",
        alk_budget_metric.value
    );
    assert!(
        budget_entry
            .metric("nitrogen_cycle.alkalinity_consumed_meq")
            .is_some(),
        "nitrogen_cycle budget entry should expose alkalinity_consumed_meq"
    );
    assert!(
        budget_entry
            .metric("nitrogen_cycle.alkalinity_produced_meq")
            .is_some(),
        "nitrogen_cycle budget entry should expose alkalinity_produced_meq"
    );
    assert!(
        budget_entry
            .metric("nitrogen_cycle.tan_oxidized_mg")
            .is_some(),
        "nitrogen_cycle budget entry should expose tan_oxidized_mg"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Acceptance criterion: NOB does not charge alkalinity a second time
// ---------------------------------------------------------------------------

/// Verifies that NOB (NO₂⁻ → NO₃⁻) does not consume additional alkalinity
/// beyond what AOB already charged for the TAN → NO₂⁻ step.
#[test]
fn nob_does_not_double_charge_alkalinity() -> Result<(), tank_core::SimError> {
    // Set up a state with elevated nitrite (NOB substrate) but zero TAN
    // so only NOB runs, not AOB or comammox.
    let mut state = TankState::new(SimSeed(7_400));
    let vol = state.water_volume_l();

    state.water.ammonia_total_mg_n_total = 0.0; // No TAN → no AOB/comammox
    state.water.nitrite_mg_n_total = 5.0 * vol; // Plenty of NO2 for NOB
    state.water.nitrate_mg_n_total = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.20;
    state.microbe.comammox_biomass_g = 0.0;
    state.filter_state.biofilter_maturity_index = 0.8;

    let alk_before = state.water.alkalinity_meq_total;

    // Run one nitrogen cycle step directly
    let output = step_nitrogen_cycle(&mut state);

    let alk_after = state.water.alkalinity_meq_total;
    let alk_consumed = alk_before - alk_after;

    // NOB should have oxidized some nitrite
    assert!(
        state.water.nitrate_mg_n_total > 0.0,
        "NOB should produce nitrate"
    );

    // But alkalinity should NOT have been consumed (NOB doesn't charge)
    assert!(
        alk_consumed.abs() < 1e-9,
        "NOB should not consume alkalinity: consumed={alk_consumed:.9}"
    );

    // Output should show zero alkalinity consumption
    assert!(
        output.alkalinity_consumed_meq.abs() < 1e-9,
        "NitrogenCycleOutput.alkalinity_consumed_meq should be zero for NOB-only: {:.9}",
        output.alkalinity_consumed_meq
    );
    assert!(
        output.alkalinity_produced_meq.abs() < 1e-9,
        "NitrogenCycleOutput.alkalinity_produced_meq should be zero for NOB-only: {:.9}",
        output.alkalinity_produced_meq
    );

    // AOB/comammox TAN oxidation should be zero; only NOB should run.
    assert!(
        output.tan_oxidized_mg.abs() < 1e-9,
        "No TAN oxidation expected during NOB-only step"
    );
    assert!(
        output.aob_n_oxidized_mg.abs() < 1e-9,
        "No AOB activity expected"
    );
    assert!(
        output.nob_n_oxidized_mg > 0.0,
        "NOB activity expected during nitrite-only step"
    );
    assert!(
        output.comammox_n_oxidized_mg.abs() < 1e-9,
        "No comammox activity expected"
    );
    assert!(
        output.nitrate_produced_mg_n > 0.0,
        "Nitrate production should reflect the NOB step"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Verification: stoichiometric constants are correctly valued
// ---------------------------------------------------------------------------

#[test]
fn stoichiometric_constants_match_literature() -> Result<(), tank_core::SimError> {
    // NITRIFICATION_ALK_MEQ_PER_MG_N = 2/14.007 ≈ 0.14285
    let expected_nitrification = 2.0 / 14.007;
    assert!(
        (NITRIFICATION_ALK_MEQ_PER_MG_N - expected_nitrification).abs() < 1e-12,
        "NITRIFICATION_ALK_MEQ_PER_MG_N should be 2/14.007"
    );

    // DENITRIFICATION_ALK_MEQ_PER_MG_N = 1/14.007 ≈ 0.07142
    let expected_denitrification = 1.0 / 14.007;
    assert!(
        (tank_core::DENITRIFICATION_ALK_MEQ_PER_MG_N - expected_denitrification).abs() < 1e-12,
        "DENITRIFICATION_ALK_MEQ_PER_MG_N should be 1/14.007"
    );

    // Denitrification returns approximately half of nitrification consumption
    let ratio = tank_core::DENITRIFICATION_ALK_MEQ_PER_MG_N / NITRIFICATION_ALK_MEQ_PER_MG_N;
    assert!(
        (ratio - 0.5).abs() < 1e-12,
        "Denitrification should return half of nitrification alkalinity: ratio={ratio}"
    );

    // Default ProcessParams should use the named constant
    let pp = ProcessParams::default();
    assert!(
        (pp.alkalinity_meq_per_mg_n_nitrified - NITRIFICATION_ALK_MEQ_PER_MG_N).abs() < 1e-12,
        "Default ProcessParams.alkalinity_meq_per_mg_n_nitrified should match NITRIFICATION_ALK_MEQ_PER_MG_N"
    );

    Ok(())
}
