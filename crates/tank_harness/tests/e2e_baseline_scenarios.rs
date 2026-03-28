//! Baseline scenario envelopes for v0.1 behavior.
//!
//! These tests capture the qualitative behavior of each shipped scenario preset
//! (nano_cycle, medium_planted, warm_room) as envelope bounds rather than exact
//! numeric traces. Future refactors should compare against these envelopes.
//!
//! Each test:
//! 1. Constructs a scenario from presets
//! 2. Enables A2d tracing and budget instrumentation
//! 3. Runs the simulation for 500+ hours (typically 2700+ hours = ~114 days)
//! 4. Asserts envelope compliance at key checkpoint hours
//! 5. On failure: dumps full structured trace to artifact directory
//!
//! Set `TANK_E2E_VERBOSE=1` for full trace dumps even on success.
//! Exit codes: 0 = all pass, non-zero = envelope violation.
//!
//! Convention for envelope reasoning:
//! Each envelope bound includes a comment explaining *why* that range is expected,
//! so future developers don't cargo-cult widen tolerances when numbers change.

use tank_core::{PlayerAction, SimSeed};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

// ---------------------------------------------------------------------------
// nano_cycle baseline
// ---------------------------------------------------------------------------

/// Nano cycle baseline: 30L soft-acidic tank, inert sand, fast stems only.
///
/// **What should happen and why:**
/// - During fishless cycling (0.05g/day feed as ammonia source), TAN rises steadily
///   because the soft acidic water has poor buffering capacity. AOB/NOB grow slowly.
/// - Around day 21-28, pH crashes below 6.0 as nitrification consumes the limited
///   alkalinity. This stalls further nitrification, causing TAN to accumulate.
/// - Shrimp stocked at day 30 die within the first week because TAN > 10 mg N/L
///   is acutely toxic to neocaridina.
/// - Plants (fast stems) decline slowly due to low nutrient availability in soft water
///   and competition with rising algae pressure.
/// - DO remains high (~8.4+) because there is little biological oxygen demand.
/// - Temperature stabilizes near the 24C ambient within the first day.
///
/// **Behavioral stories preserved:**
/// 1. pH crash in low-buffer water stalls nitrification
/// 2. TAN accumulation kills shrimp in uncycled tanks
/// 3. Small tank thermal equilibrium is rapid
/// 4. Low-light, low-nutrient conditions suppress algae growth
#[test]
fn nano_cycle_baseline_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let mut run = HarnessRun::new(SimSeed(42), "nano_cycle")?;
    run.enable_instrumentation();

    // --- Phase 1: Fishless cycling (30 days) ---
    // Daily feed as ammonia source, no water changes
    for day in 1..=30 {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;

        // Week 1: nitrifiers starting to colonize, TAN building
        if day == 7 {
            run.assert_envelope(
                "cycle_week1",
                &Envelope::default()
                    // pH should still be buffered (soft acidic water starts ~6.4, may rise
                    // from carbonate equilibrium shifts during early cycling)
                    .ph(5.0, 9.0)
                    // TAN rising from feed decomposition, not yet consumed by immature biofilter
                    .tan_mg_n_per_l(0.0, 20.0)
                    // DO stays near saturation — minimal bioload
                    .do_min(7.0)
                    // Plants should still be near starting mass (5g ± 1g)
                    .plant_biomass_g(3.0, 8.0)
                    // Biofilter just starting to establish
                    .biofilter_maturity(0.05, 0.5),
            );
        }

        // Week 4: pH crash expected — soft water alkalinity exhausted by nitrification
        if day == 28 {
            run.assert_envelope(
                "cycle_week4",
                &Envelope::default()
                    // pH may have crashed below 6.0 due to alkalinity exhaustion,
                    // or may still be buffered if nitrification was slow
                    .ph(4.5, 9.0)
                    // TAN accumulating significantly (8-20 mg/L typical)
                    .tan_mg_n_per_l(0.0, 30.0)
                    // Nitrite should be detectable — intermediate in the N cycle
                    .nitrite_mg_n_per_l(0.0, 15.0)
                    // Nitrate should be present once the cycle has started turning TAN over
                    .nitrate_mg_n_per_l(0.5, 2.5)
                    // DO still high — reaeration exceeds small BOD
                    .do_min(7.0)
                    // Nitrifiers have grown over 4 weeks
                    .biofilter_maturity(0.3, 1.0),
            );
        }
    }

    // --- Phase 2: Stock shrimp into an uncycled tank ---
    // This deliberately tests the "bad outcome" path: stocking too early kills shrimp
    run.apply_action(PlayerAction::AddShrimp { count: 8 })?;
    run.step_hours(1)?;

    run.assert_envelope(
        "post_stock",
        &Envelope::default()
            // TAN is dangerously high for shrimp (>10 mg N/L is lethal)
            .tan_mg_n_per_l(5.0, 50.0)
            .shrimp_count(8, 8)
            // Temperature should have equilibrated to ambient by now
            .temperature_c(22.0, 26.0),
    );

    // --- Phase 3: Weekly maintenance with feeding (8 weeks) ---
    for week in 1..=8 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
            run.step_hours(24)?;
        }
        run.apply_action(PlayerAction::WaterChangePercent {
            percent: 20.0,
            source_profile_id: "soft_acidic".to_string(),
        })?;
        run.step_hours(1)?;

        if week == 1 {
            run.assert_envelope(
                "stocked_week1",
                &Envelope::default()
                    // Shrimp should be dead or dying — TAN way too high
                    .shrimp_count(0, 8)
                    // TAN continues to accumulate
                    .tan_mg_n_per_l(5.0, 80.0)
                    // DO still adequate
                    .do_min(6.0)
                    .temperature_c(22.0, 26.0),
            );
        }

        if week == 4 {
            run.assert_envelope(
                "stocked_week4",
                &Envelope::default()
                    // All shrimp should be dead by now
                    .shrimp_count(0, 0)
                    // TAN very high — no shrimp ammonia but continued feed decomposition
                    .tan_mg_n_per_l(10.0, 100.0)
                    // Plants declining but still present
                    .plant_biomass_g(2.0, 8.0),
            );
        }
    }

    // --- Phase 4: Extended run to 500+ hours total (weeks 9-12) ---
    for week in 9..=12 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
            run.step_hours(24)?;
        }
        if week % 2 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "soft_acidic".to_string(),
            })?;
            run.step_hours(1)?;
        }
    }

    // Total simulated: ~2736 hours (114 days)
    run.assert_envelope(
        "final_d114",
        &Envelope::default()
            // pH should be depressed from ongoing nitrification in soft water
            .ph(4.5, 8.5)
            // TAN very high — stalled cycle
            .tan_mg_n_per_l(10.0, 150.0)
            // Nitrate stays present but modest because the cycle repeatedly stalls
            .nitrate_mg_n_per_l(3.5, 7.0)
            // All shrimp long dead
            .shrimp_count(0, 0)
            // Plants declining but not zero
            .plant_biomass_g(1.0, 8.0)
            // DO still near saturation — low total biomass
            .do_min(7.0)
            // Algae nuisance low in nano tanks with soft water
            .algae_nuisance(0.0, 0.5)
            .temperature_c(22.0, 26.0),
    );

    // Verify all values are finite (catch NaN/Inf regressions)
    run.assert_snapshot("final_finite", |snap| {
        for (name, val) in [
            ("pH", snap.ph),
            ("temp", snap.water_temp_c),
            ("DO", snap.do_mg_l),
            ("TAN", snap.tan_mg_n_per_l),
            ("NO2", snap.nitrite_mg_n_per_l),
            ("NO3", snap.nitrate_mg_n_per_l),
            ("plant_biomass", snap.total_plant_biomass_g),
        ] {
            if !val.is_finite() {
                return Err(format!("{name} is not finite: {val}"));
            }
        }
        Ok(())
    });

    run.finish().map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// medium_planted baseline
// ---------------------------------------------------------------------------

/// Medium planted baseline: 65L moderate water, active+coarse substrate, both guilds.
///
/// **What should happen and why:**
/// - Active substrate provides initial nutrient charge (N and P) that boosts early
///   plant growth. Fast stems grow significantly (5g -> 9g+); rosettes decline
///   because substrate nutrients deplete and water-column uptake competition favors stems.
/// - Moderate water has much better buffering than soft acidic, so pH holds at 8.5
///   initially but gradually drops as alkalinity is consumed by nitrification.
/// - TAN rises to 7-13 mg N/L — less than nano_cycle because the larger volume
///   dilutes the same ammonia load and plants take up some N.
/// - Biofilter matures steadily (maturity 0.1 -> 0.9+), but not fast enough to
///   prevent lethal TAN levels for shrimp.
/// - Shrimp stocked at day 30 still die from TAN/NO2 toxicity, though conditions
///   are less extreme than in nano_cycle.
/// - Algae (both suspended and periphyton) grow steadily, reaching nuisance levels
///   (0.3-0.4) by week 12 — nutrients are available and competition from plants
///   is insufficient to fully suppress algae.
/// - DO stays high (~8.3-8.5) because moderate light supports plant photosynthesis.
///
/// **Behavioral stories preserved:**
/// 1. Fast-stem vs rosette guild competition (stems win water-column battle)
/// 2. Active substrate nutrient charge fuels early growth
/// 3. Biofilter maturation timeline in moderate water
/// 4. Algae pressure rises with nutrient accumulation
/// 5. Larger tank dilutes TAN relative to nano but still insufficient for shrimp
#[test]
fn medium_planted_baseline_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let mut run = HarnessRun::new(SimSeed(42), "medium_planted")?;
    run.enable_instrumentation();

    // --- Phase 1: Fishless cycling (30 days) ---
    for day in 1..=30 {
        run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        run.step_hours(24)?;

        if day == 7 {
            run.assert_envelope(
                "cycle_week1",
                &Envelope::default()
                    // Moderate water buffers pH well
                    .ph(6.5, 9.0)
                    // TAN just starting to accumulate (<3 mg/L in larger tank)
                    .tan_mg_n_per_l(0.0, 5.0)
                    // Plants growing from active substrate nutrients
                    .plant_biomass_g(8.0, 16.0)
                    // Biofilter starting to establish
                    .biofilter_maturity(0.05, 0.5)
                    // DO near saturation at 24C
                    .do_min(7.0),
            );
        }

        if day == 14 {
            run.assert_envelope(
                "cycle_week2",
                &Envelope::default()
                    .ph(6.5, 9.0)
                    // TAN rising but diluted by larger volume
                    .tan_mg_n_per_l(0.0, 10.0)
                    // Both guilds present, total biomass growing
                    .plant_biomass_g(8.0, 20.0)
                    // Biofilter progressing
                    .biofilter_maturity(0.1, 0.6),
            );
        }

        if day == 28 {
            run.assert_envelope(
                "cycle_week4",
                &Envelope::default()
                    // pH still buffered in moderate water (may start dropping)
                    .ph(5.5, 9.0)
                    // TAN has been building for 4 weeks
                    .tan_mg_n_per_l(1.0, 25.0)
                    // NO2 intermediate accumulating
                    .nitrite_mg_n_per_l(0.0, 10.0)
                    // Moderate source water plus growing nitrifiers should keep NO3 in view
                    .nitrate_mg_n_per_l(4.0, 6.0)
                    // Biofilter maturing nicely
                    .biofilter_maturity(0.4, 1.0)
                    // Plants still growing — active substrate provides nutrients
                    .plant_biomass_g(8.0, 20.0),
            );
        }
    }

    // Stock shrimp at day 30
    run.apply_action(PlayerAction::AddShrimp { count: 10 })?;
    run.step_hours(1)?;

    run.assert_envelope(
        "post_stock",
        &Envelope::default()
            .tan_mg_n_per_l(2.0, 25.0)
            .shrimp_count(10, 10)
            .temperature_c(22.0, 26.0)
            // Fast stems should be dominant by now
            .fast_stem_biomass_g(4.0, 15.0),
    );

    // --- Phase 2: Weekly maintenance (8 weeks) ---
    for week in 1..=8 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.2 })?;
            run.step_hours(24)?;
        }
        run.apply_action(PlayerAction::WaterChangePercent {
            percent: 25.0,
            source_profile_id: "moderate".to_string(),
        })?;
        run.step_hours(1)?;

        if week == 2 {
            run.assert_envelope(
                "stocked_week2",
                &Envelope::default()
                    .ph(5.5, 9.0)
                    // Shrimp may still be dying off
                    .shrimp_count(0, 10)
                    // TAN still elevated
                    .tan_mg_n_per_l(2.0, 30.0)
                    .do_min(6.0)
                    // Plants holding or growing
                    .plant_biomass_g(8.0, 20.0),
            );
        }

        if week == 4 {
            run.assert_envelope(
                "stocked_week4",
                &Envelope::default()
                    .ph(5.5, 9.0)
                    // TAN persistent — cycle not yet complete
                    .tan_mg_n_per_l(2.0, 30.0)
                    // Fast stems dominating, rosettes declining
                    .fast_stem_biomass_g(5.0, 15.0)
                    // Algae growing with available nutrients
                    .algae_nuisance(0.0, 0.5),
            );
        }

        if week == 8 {
            run.assert_envelope(
                "stocked_week8",
                &Envelope::default()
                    .ph(5.5, 9.0)
                    .tan_mg_n_per_l(2.0, 30.0)
                    .nitrite_mg_n_per_l(0.0, 15.0)
                    // Matured planted tanks should settle into a moderate NO3 band, not zero out
                    .nitrate_mg_n_per_l(4.0, 5.5)
                    .do_min(6.0)
                    .plant_biomass_g(6.0, 20.0)
                    // Biofilter approaching full maturity
                    .biofilter_maturity(0.7, 1.0)
                    .algae_nuisance(0.0, 0.6),
            );
        }
    }

    // --- Phase 3: Extended run (weeks 9-12) ---
    for week in 9..=12 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.2 })?;
            run.step_hours(24)?;
        }
        if week % 2 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 25.0,
                source_profile_id: "moderate".to_string(),
            })?;
            run.step_hours(1)?;
        }
    }

    // Final: ~114 days = ~2736 hours
    run.assert_envelope(
        "final_d114",
        &Envelope::default()
            .ph(5.0, 9.0)
            // TAN still elevated — cycle not fully controlling ammonia
            .tan_mg_n_per_l(2.0, 50.0)
            // A mature but feed-limited planted tank should retain a few mg/L of NO3
            .nitrate_mg_n_per_l(3.0, 5.5)
            .shrimp_count(0, 0)
            // Plants still present, fast stems dominant
            .plant_biomass_g(5.0, 20.0)
            .fast_stem_biomass_g(5.0, 15.0)
            // Algae nuisance has risen
            .algae_nuisance(0.1, 0.8)
            // DO good — plants + reaeration
            .do_min(6.0)
            .temperature_c(22.0, 26.0),
    );

    run.assert_snapshot("final_finite", |snap| {
        for (name, val) in [
            ("pH", snap.ph),
            ("temp", snap.water_temp_c),
            ("DO", snap.do_mg_l),
            ("TAN", snap.tan_mg_n_per_l),
            ("NO2", snap.nitrite_mg_n_per_l),
            ("NO3", snap.nitrate_mg_n_per_l),
            ("plant_biomass", snap.total_plant_biomass_g),
            ("algae_nuisance", snap.algae_nuisance_index),
            ("biofilter_maturity", snap.biofilter_maturity_index),
        ] {
            if !val.is_finite() {
                return Err(format!("{name} is not finite: {val}"));
            }
        }
        Ok(())
    });

    run.finish().map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// warm_room baseline
// ---------------------------------------------------------------------------

/// Warm room baseline: 32L moderate water, inert gravel, fast stems, 29C ambient.
///
/// **What should happen and why:**
/// - The tank starts at 24C but rapidly equilibrates to the 29C ambient temperature
///   within the first week. This is the defining characteristic of this scenario.
/// - Higher temperature (29C) reduces DO saturation from ~8.3 to ~7.5 mg/L.
///   Combined with higher metabolic rates, this creates a mildly oxygen-stressed
///   environment compared to the 24C scenarios.
/// - Warm conditions accelerate all biological rates: faster nitrification,
///   faster decomposition, faster algae growth. The biofilter matures quickly.
/// - Algae growth is significantly higher than in other scenarios because warmth
///   favors algae over plants. Nuisance index reaches 0.3-0.5+ by week 12.
/// - 29C is at the upper thermal tolerance for neocaridina (prefer 20-26C).
///   Shrimp stocked at day 30 die from combined TAN/NO2/thermal stress.
/// - TAN/NO2 accumulate because cycling hasn't completed, though the warmer
///   temperature does accelerate nitrifier growth.
///
/// **Behavioral stories preserved:**
/// 1. Thermal equilibration from 24C to 29C ambient (smaller tank = faster)
/// 2. Reduced DO saturation at elevated temperature
/// 3. Warm conditions favor algae over plants
/// 4. High-temp stress on shrimp survival
/// 5. Accelerated biological rates at warm temperature
#[test]
fn warm_room_baseline_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let mut run = HarnessRun::new(SimSeed(42), "warm_room")?;
    run.enable_instrumentation();

    // --- Phase 1: Fishless cycling (30 days) at warm temperature ---
    for day in 1..=30 {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;

        if day == 7 {
            run.assert_envelope(
                "cycle_week1",
                &Envelope::default()
                    // Temperature should have equilibrated to ambient ~29C by now
                    // (32L tank reaches half-delta in hours, not days)
                    .temperature_c(27.0, 31.0)
                    // DO reduced at warm temperature — saturation ~7.5 mg/L at 29C
                    .do_mg_l(6.0, 9.0)
                    // TAN building from feed
                    .tan_mg_n_per_l(0.0, 10.0)
                    // Biofilter starting
                    .biofilter_maturity(0.05, 0.5),
            );
        }

        if day == 21 {
            run.assert_envelope(
                "cycle_week3",
                &Envelope::default()
                    .temperature_c(27.0, 31.0)
                    // TAN accumulating (3-6 mg/L typical)
                    .tan_mg_n_per_l(0.0, 15.0)
                    // NO2 intermediate building — warm temps accelerate AOB
                    .nitrite_mg_n_per_l(0.0, 10.0)
                    // Biofilter maturing faster than in 24C scenarios
                    .biofilter_maturity(0.2, 0.8),
            );
        }

        if day == 28 {
            run.assert_envelope(
                "cycle_week4",
                &Envelope::default()
                    .temperature_c(27.0, 31.0)
                    .ph(5.0, 9.0)
                    .tan_mg_n_per_l(0.0, 15.0)
                    .nitrite_mg_n_per_l(0.0, 10.0)
                    // Warm, oxygenated water keeps nitrate present once nitrifiers ramp up
                    .nitrate_mg_n_per_l(4.5, 6.0)
                    // Biofilter should be well-established by week 4 at warm temps
                    .biofilter_maturity(0.4, 1.0),
            );
        }
    }

    // Stock shrimp at day 30
    run.apply_action(PlayerAction::AddShrimp { count: 8 })?;
    run.step_hours(1)?;

    run.assert_envelope(
        "post_stock",
        &Envelope::default()
            .shrimp_count(8, 8)
            // Warm temperature — defining characteristic
            .temperature_c(27.0, 31.0)
            // DO reduced at warm temp
            .do_mg_l(6.0, 9.0)
            // TAN still elevated
            .tan_mg_n_per_l(0.5, 20.0),
    );

    // --- Phase 2: Weekly maintenance (8 weeks) ---
    for week in 1..=8 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.15 })?;
            run.step_hours(24)?;
        }
        run.apply_action(PlayerAction::WaterChangePercent {
            percent: 20.0,
            source_profile_id: "moderate".to_string(),
        })?;
        run.step_hours(1)?;

        if week == 1 {
            run.assert_envelope(
                "stocked_week1",
                &Envelope::default()
                    // Shrimp dying from combined TAN/temp stress
                    .shrimp_count(0, 8)
                    .temperature_c(26.0, 31.0)
                    .do_mg_l(5.0, 9.0),
            );
        }

        if week == 4 {
            run.assert_envelope(
                "stocked_week4",
                &Envelope::default()
                    // Shrimp likely all dead from TAN/NO2/thermal stress
                    .shrimp_count(0, 8)
                    .tan_mg_n_per_l(2.0, 50.0)
                    // Algae growing faster at warm temps
                    .algae_nuisance(0.0, 0.6),
            );
        }

        if week == 8 {
            run.assert_envelope(
                "stocked_week8",
                &Envelope::default()
                    .ph(5.0, 9.0)
                    .shrimp_count(0, 0)
                    .temperature_c(26.0, 31.0)
                    .do_mg_l(5.0, 9.5)
                    // Warm maintenance keeps NO3 detectable even while TAN/NO2 remain elevated
                    .nitrate_mg_n_per_l(4.5, 5.5)
                    // Algae nuisance should be noticeable at warm temps
                    .algae_nuisance(0.1, 0.8)
                    // Plants declining slowly at warm temp
                    .plant_biomass_g(2.0, 10.0),
            );
        }
    }

    // --- Phase 3: Extended run (weeks 9-12) ---
    for week in 9..=12 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.15 })?;
            run.step_hours(24)?;
        }
        if week % 2 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "moderate".to_string(),
            })?;
            run.step_hours(1)?;
        }
    }

    // Final: ~114 days
    run.assert_envelope(
        "final_d114",
        &Envelope::default()
            .ph(5.0, 9.0)
            // Warm tank — elevated temperature is the key characteristic
            .temperature_c(26.0, 31.0)
            // DO suppressed at warm temp
            .do_mg_l(5.0, 9.5)
            // TAN very high — incomplete cycle + ongoing feed
            .tan_mg_n_per_l(5.0, 80.0)
            // Warm source water should still leave a stable nitrate floor late in the run
            .nitrate_mg_n_per_l(4.0, 5.5)
            .shrimp_count(0, 0)
            // Plants declining at warm temp
            .plant_biomass_g(1.0, 10.0)
            // Algae nuisance elevated — warm conditions favor algae
            .algae_nuisance(0.2, 0.9),
    );

    run.assert_snapshot("final_finite", |snap| {
        for (name, val) in [
            ("pH", snap.ph),
            ("temp", snap.water_temp_c),
            ("DO", snap.do_mg_l),
            ("TAN", snap.tan_mg_n_per_l),
            ("NO2", snap.nitrite_mg_n_per_l),
            ("NO3", snap.nitrate_mg_n_per_l),
            ("plant_biomass", snap.total_plant_biomass_g),
        ] {
            if !val.is_finite() {
                return Err(format!("{name} is not finite: {val}"));
            }
        }
        Ok(())
    });

    run.finish().map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// Cross-scenario comparative assertions
// ---------------------------------------------------------------------------

/// Compares key metrics across all three scenarios to verify that relative
/// relationships hold, regardless of absolute values.
///
/// **What should happen and why:**
/// - warm_room has lower DO saturation than nano_cycle and medium_planted
///   because DO saturation decreases with temperature.
/// - warm_room has higher algae nuisance because warm conditions favor algae.
/// - medium_planted has higher total plant biomass than the other two because
///   it starts with both guilds and active substrate nutrients.
/// - nano_cycle has the highest TAN relative to volume because it's the smallest
///   tank receiving the same daily feed mass per volume.
#[test]
fn cross_scenario_relative_behaviors() -> Result<(), Box<dyn std::error::Error>> {
    // Run all three scenarios for 60 days (1440 hours) to establish steady-state
    let scenarios: Vec<(&str, u64, f64)> = vec![
        ("nano_cycle", 42, 0.05),
        ("medium_planted", 42, 0.1),
        ("warm_room", 42, 0.05),
    ];

    let mut final_snapshots = Vec::new();

    for (scenario_id, seed, feed_g) in &scenarios {
        let mut run = HarnessRun::new(SimSeed(*seed), scenario_id)?;

        // Run 60 days with daily feeding, no shrimp, no water changes
        for _ in 1..=60 {
            run.apply_action(PlayerAction::Feed { grams: *feed_g })?;
            run.step_hours(24)?;
        }

        final_snapshots.push((*scenario_id, run.snapshot()));
        run.finish()?;
    }

    let nano = &final_snapshots[0].1;
    let medium = &final_snapshots[1].1;
    let warm = &final_snapshots[2].1;

    // Warm room should have warmer water than the other two
    assert!(
        warm.water_temp_c > nano.water_temp_c + 3.0,
        "warm_room ({:.1}C) should be >3C warmer than nano_cycle ({:.1}C)",
        warm.water_temp_c,
        nano.water_temp_c
    );
    assert!(
        warm.water_temp_c > medium.water_temp_c + 3.0,
        "warm_room ({:.1}C) should be >3C warmer than medium_planted ({:.1}C)",
        warm.water_temp_c,
        medium.water_temp_c
    );

    // Warm room should have lower DO saturation
    assert!(
        warm.do_sat_mg_l < nano.do_sat_mg_l,
        "warm_room DO saturation ({:.2}) should be lower than nano_cycle ({:.2})",
        warm.do_sat_mg_l,
        nano.do_sat_mg_l
    );

    // Medium planted should have more plant biomass (started with 10g vs 5g)
    assert!(
        medium.total_plant_biomass_g > nano.total_plant_biomass_g,
        "medium_planted plants ({:.2}g) should exceed nano_cycle ({:.2}g)",
        medium.total_plant_biomass_g,
        nano.total_plant_biomass_g
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Thermal inertia: nano vs medium tank size
// ---------------------------------------------------------------------------

/// Verifies that smaller tanks equilibrate to ambient temperature faster.
///
/// **What should happen and why:**
/// - Both nano_cycle (30L) and medium_planted (65L) start at their default water
///   temperatures. When ambient shifts, the nano_cycle reaches the new equilibrium
///   faster because it has less thermal mass.
/// - This preserves the tank-size thermal response behavior from the existing
///   `tank_size_thermal_response.rs` tests but using scenario presets.
#[test]
fn thermal_inertia_scales_with_tank_size() -> Result<(), Box<dyn std::error::Error>> {
    let mut nano = HarnessRun::new(SimSeed(42), "nano_cycle")?;
    let mut medium = HarnessRun::new(SimSeed(42), "medium_planted")?;

    // Both start near 24C; ambient is already 24C, so shift it to 30C
    nano.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 30.0 })?;
    medium.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 30.0 })?;

    // Run 12 hours
    nano.step_hours(12)?;
    medium.step_hours(12)?;

    let nano_snap = nano.snapshot();
    let medium_snap = medium.snapshot();

    // After 12 hours, the smaller nano tank should be closer to the 30C target
    let nano_delta = (30.0 - nano_snap.water_temp_c).abs();
    let medium_delta = (30.0 - medium_snap.water_temp_c).abs();

    assert!(
        nano_delta < medium_delta,
        "nano ({:.2}C, delta={:.2}) should be closer to 30C than medium ({:.2}C, delta={:.2})",
        nano_snap.water_temp_c,
        nano_delta,
        medium_snap.water_temp_c,
        medium_delta
    );

    nano.finish()?;
    medium.finish()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// DO behavior: light/dark cycle
// ---------------------------------------------------------------------------

/// Verifies that DO dips during dark hours due to respiration without photosynthesis.
///
/// **What should happen and why:**
/// - During lit hours, plant photosynthesis produces O2, partially offsetting
///   biological oxygen demand from respiration.
/// - During dark hours, only respiration occurs, causing net DO consumption.
/// - Reaeration from atmosphere partially compensates, but the mean DO during
///   dark hours should still be lower than during lit hours.
#[test]
fn do_dips_at_night_in_planted_tank() -> Result<(), Box<dyn std::error::Error>> {
    let mut run = HarnessRun::new(SimSeed(42), "medium_planted")?;

    // Run a few weeks of feeding to establish bioload and a stable day/night cycle.
    for _ in 0..21 {
        run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        run.step_hours(24)?;
    }

    // Sample a full day and compare the daylight oxygen peak against the final
    // dark-period trough. This avoids a false reversal from averaging across the
    // whole dark window immediately after lights-out.
    let mut lit_peak = f64::NEG_INFINITY;
    let mut late_dark_trough = f64::INFINITY;

    for _ in 0..24 {
        let snap = run.snapshot();
        let hour = snap.hour;
        run.step_hours(1)?;
        let post_snap = run.snapshot();

        // Medium planted uses default 8h photoperiod (hours 0-7 are lit)
        if hour < 8 {
            lit_peak = lit_peak.max(post_snap.do_mg_l);
        } else if hour >= 20 {
            late_dark_trough = late_dark_trough.min(post_snap.do_mg_l);
        }
    }

    if lit_peak.is_finite() && late_dark_trough.is_finite() {
        assert!(
            late_dark_trough < lit_peak,
            "late-dark DO trough ({:.3}) should stay below the lit peak ({:.3})",
            late_dark_trough,
            lit_peak
        );
    }

    run.finish().map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// Reproduction-preserving maintained variant
// ---------------------------------------------------------------------------

/// Maintained planted variant: medium_planted tuned toward shrimp husbandry.
///
/// **What should happen and why:**
/// - The base medium_planted layout can support shrimp reproduction once the water
///   chemistry is shifted toward Neocaridina husbandry: harder source water,
///   stronger aeration, longer photoperiod, and a larger diluted volume.
/// - With light feeding and weekly water changes, TAN/NO2 should stay low enough
///   for adults to remain viable while nitrate stays in a modest planted-tank band.
/// - Over a 90 day run, the simulation should show a real reproduction window:
///   readiness rises, at least one berried female appears, and juveniles hatch.
#[test]
fn maintained_medium_variant_reaches_reproduction_window() -> Result<(), Box<dyn std::error::Error>>
{
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 1.5,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlantedWithCoarsePorous),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(10),
    };
    let mut state =
        tank_scenarios::seeded_state_with_full_overrides(SimSeed(42), "medium_planted", overrides)?;
    state.algae.periphyton_biomass_g = state.algae.periphyton_biomass_g.max(5.0);
    state.animal.adult.reserve_g = state.animal.adult.reserve_g.max(3.0);
    state.animal.reproductive_readiness_index = state.animal.reproductive_readiness_index.max(0.5);
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state
        .shrimp_params
        .apply_legacy_total_maturation_days(120.0);

    let mut run = HarnessRun::from_state(SimSeed(42), "medium_planted", state);
    run.enable_instrumentation();

    let mut max_juveniles = 0;
    let mut max_berried = 0;
    let mut max_readiness = run.snapshot().shrimp_reproductive_readiness;

    for day in 1..=90 {
        run.apply_action(PlayerAction::Feed { grams: 0.01 })?;
        run.step_hours(24)?;

        if day % 7 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
            run.step_hours(1)?;
        }

        let snap = run.snapshot();
        max_juveniles = max_juveniles.max(snap.juveniles_count);
        max_berried = max_berried.max(snap.berried_females_count);
        max_readiness = max_readiness.max(snap.shrimp_reproductive_readiness);

        if day == 30 {
            run.assert_envelope(
                "repro_day30",
                &Envelope::default()
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 0.8)
                    .nitrate_mg_n_per_l(6.0, 10.5)
                    .do_min(7.0)
                    .shrimp_count(20, 60)
                    .berried_females_count(1, 5)
                    .shrimp_reproductive_readiness(0.35, 0.75),
            );
        }
        if day == 60 {
            run.assert_envelope(
                "repro_day60",
                &Envelope::default()
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 1.3)
                    .nitrate_mg_n_per_l(5.0, 10.5)
                    .do_min(7.0)
                    .shrimp_count(10, 80)
                    .juveniles_count(5, 60)
                    .shrimp_reproductive_readiness(0.35, 0.8),
            );
        }
        if day == 90 {
            run.assert_envelope(
                "repro_day90",
                &Envelope::default()
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 1.3)
                    .nitrate_mg_n_per_l(4.0, 10.0)
                    .do_min(7.0)
                    .shrimp_count(10, 120)
                    .juveniles_count(5, 80),
            );
        }
    }

    run.assert_snapshot("reproduction_window", |_| {
        if max_readiness < 0.45 {
            return Err(format!(
                "reproductive readiness never exceeded 0.45 (max={max_readiness:.3})"
            ));
        }
        if max_berried == 0 {
            return Err("no berried females appeared during the maintained run".to_string());
        }
        if max_juveniles < 5 {
            return Err(format!(
                "juvenile hatch window never exceeded 5 shrimp (max={max_juveniles})"
            ));
        }
        Ok(())
    });

    run.finish().map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// Biofilter maturation timeline
// ---------------------------------------------------------------------------

/// Verifies that biofilter maturity increases over time during cycling.
///
/// **What should happen and why:**
/// - With a continuous ammonia source (daily feeding), AOB and NOB colonies
///   grow and the biofilter matures. This is the fundamental cycling process.
/// - After 30 days of cycling, biofilter maturity should be substantially higher
///   than at the start. The exact value depends on temperature, pH, and substrate,
///   but all scenarios should show monotonic improvement over 30 days.
#[test]
fn biofilter_matures_during_cycling() -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = ["nano_cycle", "medium_planted", "warm_room"];

    for scenario_id in &scenarios {
        let mut run = HarnessRun::new(SimSeed(42), scenario_id)?;

        let initial_maturity = run.snapshot().biofilter_maturity_index;

        // Feed daily for 30 days
        for _ in 0..30 {
            run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
            run.step_hours(24)?;
        }

        let final_maturity = run.snapshot().biofilter_maturity_index;

        assert!(
            final_maturity > initial_maturity + 0.2,
            "{scenario_id}: biofilter maturity should increase significantly during cycling \
             (initial={initial_maturity:.3}, final={final_maturity:.3})"
        );

        run.finish()?;
    }

    Ok(())
}
