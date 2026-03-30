//! Temporary discovery test: runs each scenario and prints snapshot values at checkpoints.
//! Used to capture current v0.1 behavior for envelope design. Not a regression test.

use tank_core::{PlayerAction, SimSeed};
use tank_harness::HarnessRun;

fn print_checkpoint(label: &str, run: &HarnessRun) {
    let s = run.snapshot();
    eprintln!(
        "  [{label}] d{:03}h{:02} | pH={:.2} T={:.2}C DO={:.2} TAN={:.4} NO2={:.4} NO3={:.4} PO4={:.4} \
         shrimp={} (A{}/SA{}/J{}/B{}) cond={:.3} repro={:.3} \
         plant={:.3}g (FS={:.3} RR={:.3}) algae_s={:.4} peri={:.4} nui={:.3} \
         AOB={:.5} NOB={:.5} COM={:.5} DEC={:.5} maturity={:.3} \
         det_p={:.4} det_f={:.4}",
        s.day, s.hour,
        s.ph, s.water_temp_c, s.do_mg_l,
        s.tan_mg_n_per_l, s.nitrite_mg_n_per_l, s.nitrate_mg_n_per_l, s.phosphate_mg_p_per_l,
        s.total_shrimp_count, s.adult_shrimp_count, s.sub_adult_count, s.juveniles_count, s.berried_females_count,
        s.shrimp_condition_index, s.shrimp_reproductive_readiness,
        s.total_plant_biomass_g, s.fast_stem_biomass_g, s.root_feeding_rosette_biomass_g,
        s.suspended_algae_biomass_g, s.periphyton_biomass_g, s.algae_nuisance_index,
        s.ammonia_oxidizer_biomass_g, s.nitrite_oxidizer_biomass_g, s.comammox_biomass_g, s.decomposer_biomass_g,
        s.biofilter_maturity_index,
        s.detritus_particulate_g_total, s.detritus_fine_g_total,
    );
}

/// Run a scenario with fishless cycling (feeding only) for 30 days, then stock shrimp
/// and maintain with weekly water changes for 60 days. Total ~90 days = 2160 hours.
fn run_standard_lifecycle(
    scenario_id: &str,
    seed: u64,
    feed_g_cycle: f64,
    feed_g_stocked: f64,
    initial_shrimp: u32,
    wc_percent: f64,
    source_water: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("\n=== SCENARIO: {scenario_id} (seed={seed}) ===");

    let mut run = HarnessRun::new(SimSeed(seed), scenario_id)?
        .with_artifact_label(format!("{scenario_id}_discovery"));

    print_checkpoint("initial", &run);

    // Phase 1: Fishless cycle (30 days, daily feed as ammonia source)
    for day in 1..=30 {
        run.apply_action(PlayerAction::Feed {
            grams: feed_g_cycle,
        })?;
        run.step_hours(24)?;
        if day % 7 == 0 {
            print_checkpoint(&format!("cycle_d{day:02}"), &run);
        }
    }
    print_checkpoint("post_cycle_d30", &run);

    // Phase 2: Stock shrimp
    if initial_shrimp > 0 {
        run.apply_action(PlayerAction::AddShrimp {
            count: initial_shrimp,
        })?;
        run.step_hours(1)?;
        print_checkpoint("post_stock", &run);
    }

    // Phase 3: Stocked with weekly maintenance (60 days = ~8.5 weeks)
    for week in 1..=8 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed {
                grams: feed_g_stocked,
            })?;
            run.step_hours(24)?;
        }
        if wc_percent > 0.0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: wc_percent,
                source_profile_id: source_water.to_string(),
            })?;
            run.step_hours(1)?;
        }
        print_checkpoint(&format!("week_{week:02}"), &run);
    }

    // Phase 4: Continue another 30 days to hit 500+ hours with reduced maintenance
    for week in 9..=12 {
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed {
                grams: feed_g_stocked,
            })?;
            run.step_hours(24)?;
        }
        if wc_percent > 0.0 && week % 2 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: wc_percent,
                source_profile_id: source_water.to_string(),
            })?;
            run.step_hours(1)?;
        }
        print_checkpoint(&format!("week_{week:02}"), &run);
    }

    run.finish()?;
    Ok(())
}

#[test]
fn discover_nano_cycle() -> Result<(), Box<dyn std::error::Error>> {
    run_standard_lifecycle("nano_cycle", 42, 0.05, 0.1, 8, 20.0, "soft_acidic")
}

#[test]
fn discover_medium_planted() -> Result<(), Box<dyn std::error::Error>> {
    run_standard_lifecycle("medium_planted", 42, 0.1, 0.2, 10, 25.0, "moderate")
}

#[test]
fn discover_warm_room() -> Result<(), Box<dyn std::error::Error>> {
    run_standard_lifecycle("warm_room", 42, 0.05, 0.15, 8, 20.0, "moderate")
}
