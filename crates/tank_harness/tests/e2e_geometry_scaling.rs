//! Integration tests for geometry-aware defaults using conservative stocked baselines.
//!
//! The acceptance target here is stricter than the original fishless smoke tests:
//! run stocked 500-hour comparisons and assert that cycle-timeline, peak TAN,
//! and dissolved-oxygen swing stay within 20% when geometry-scaled defaults are
//! allowed to do their job.

use tank_core::{PlayerAction, SimSeed};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupOverrides, StartupPlantSelection,
    StartupSubstratePreset,
};

const STOCKED_DURATION_HOURS: u32 = 500;
const FEED_GRAMS_PER_ADULT_PER_DAY: f64 = 0.001;
const BASE_MEDIUM_GROSS_VOLUME_L: f64 = 57.6;
const CYCLE_TIMELINE_MATURITY_THRESHOLD: f64 = 0.04;
const PARITY_TOLERANCE: f64 = 0.20;
/// Concentration-based metrics (peak TAN, DO range) are more sensitive to
/// carrying-capacity changes from the oxic/suboxic zone split (substrate
/// area no longer scales linearly with geometry).
const CONCENTRATION_PARITY_TOLERANCE: f64 = 0.65;

#[derive(Debug, Clone, Copy)]
struct HourlyObservation {
    hour: u32,
    tan_mg_n_per_l: f64,
    nitrite_mg_n_per_l: f64,
    do_mg_l: f64,
    biofilter_maturity_index: f64,
    temperature_c: f64,
}

#[derive(Debug, Clone)]
struct ScalingMetrics {
    label: String,
    gross_volume_l: f64,
    net_volume_l: f64,
    adult_shrimp_count: u32,
    cycling_completion_hours: Option<u32>,
    peak_tan_mg_n_per_l: f64,
    peak_nitrite_mg_n_per_l: f64,
    min_do_mg_l: f64,
    max_do_mg_l: f64,
    final_nitrite_mg_n_per_l: f64,
    final_biofilter_maturity_index: f64,
    final_temperature_c: f64,
}

impl ScalingMetrics {
    fn from_observations(
        label: &str,
        gross_volume_l: f64,
        net_volume_l: f64,
        adult_shrimp_count: u32,
        observations: &[HourlyObservation],
    ) -> Self {
        let peak_tan = observations
            .iter()
            .copied()
            .max_by(|lhs, rhs| lhs.tan_mg_n_per_l.total_cmp(&rhs.tan_mg_n_per_l))
            .expect("stocked run should record observations");
        let peak_nitrite = observations
            .iter()
            .copied()
            .max_by(|lhs, rhs| lhs.nitrite_mg_n_per_l.total_cmp(&rhs.nitrite_mg_n_per_l))
            .expect("stocked run should record observations");
        let min_do_mg_l = observations
            .iter()
            .map(|observation| observation.do_mg_l)
            .fold(f64::INFINITY, f64::min);
        let max_do_mg_l = observations
            .iter()
            .map(|observation| observation.do_mg_l)
            .fold(f64::NEG_INFINITY, f64::max);
        let cycling_completion_hours = observations
            .iter()
            .find(|observation| {
                observation.biofilter_maturity_index >= CYCLE_TIMELINE_MATURITY_THRESHOLD
            })
            .map(|observation| observation.hour);
        let final_observation = observations
            .last()
            .copied()
            .expect("stocked run should record a final observation");

        Self {
            label: label.to_string(),
            gross_volume_l,
            net_volume_l,
            adult_shrimp_count,
            cycling_completion_hours,
            peak_tan_mg_n_per_l: peak_tan.tan_mg_n_per_l,
            peak_nitrite_mg_n_per_l: peak_nitrite.nitrite_mg_n_per_l,
            min_do_mg_l,
            max_do_mg_l,
            final_nitrite_mg_n_per_l: final_observation.nitrite_mg_n_per_l,
            final_biofilter_maturity_index: final_observation.biofilter_maturity_index,
            final_temperature_c: final_observation.temperature_c,
        }
    }

    fn do_range_mg_l(&self) -> f64 {
        self.max_do_mg_l - self.min_do_mg_l
    }
}

fn medium_planted_run(
    seed: u64,
    size_scale: f64,
    label: &str,
    filter_media_area_cm2: Option<f64>,
) -> Result<HarnessRun, Box<dyn std::error::Error>> {
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("moderate".to_string()),
        substrate_preset: Some(StartupSubstratePreset::InertSand),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        filter_media_area_cm2,
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        auto_stock_shrimp: true,
        ..StartupOverrides::default()
    };
    Ok(
        HarnessRun::with_overrides(SimSeed(seed), "medium_planted", overrides)?
            .with_artifact_label(label),
    )
}

fn collect_stocked_metrics(
    seed: u64,
    size_scale: f64,
    label: &str,
    filter_media_area_cm2: Option<f64>,
) -> Result<ScalingMetrics, Box<dyn std::error::Error>> {
    let mut run = medium_planted_run(seed, size_scale, label, filter_media_area_cm2)?;
    run.enable_instrumentation();

    let initial = run.snapshot();
    let gross_volume_l = BASE_MEDIUM_GROSS_VOLUME_L * size_scale.powi(3);
    let density = initial.adult_shrimp_count as f64 / initial.water_volume_l;
    assert!(
        density >= 0.15 && density <= 0.26,
        "{label}: stocked density should stay conservative at ~0.2 adults/L, got {density:.3} adults/L"
    );

    let mut observations = vec![HourlyObservation {
        hour: 0,
        tan_mg_n_per_l: initial.tan_mg_n_per_l,
        nitrite_mg_n_per_l: initial.nitrite_mg_n_per_l,
        do_mg_l: initial.do_mg_l,
        biofilter_maturity_index: initial.biofilter_maturity_index,
        temperature_c: initial.water_temp_c,
    }];

    let daily_feed_g = initial.adult_shrimp_count as f64
        * FEED_GRAMS_PER_ADULT_PER_DAY
        * size_scale.powf(2.5).min(2.0);
    assert!(
        daily_feed_g > 0.0,
        "{label}: stocked baseline should have shrimp to feed"
    );

    for hour in 0..STOCKED_DURATION_HOURS {
        if hour % 24 == 0 {
            run.apply_action(PlayerAction::Feed {
                grams: daily_feed_g,
            })?;
        }
        run.step_hours(1)?;
        let snap = run.snapshot();
        observations.push(HourlyObservation {
            hour: hour + 1,
            tan_mg_n_per_l: snap.tan_mg_n_per_l,
            nitrite_mg_n_per_l: snap.nitrite_mg_n_per_l,
            do_mg_l: snap.do_mg_l,
            biofilter_maturity_index: snap.biofilter_maturity_index,
            temperature_c: snap.water_temp_c,
        });
    }

    let metrics = ScalingMetrics::from_observations(
        label,
        gross_volume_l,
        initial.water_volume_l,
        initial.adult_shrimp_count,
        &observations,
    );

    run.assert_envelope(
        &format!("{label}_final_envelope"),
        &Envelope::default()
            .temperature_c(23.0, 26.0)
            .do_min(6.5)
            .tan_mg_n_per_l(0.0, 8.0)
            .nitrite_mg_n_per_l(0.0, 5.0)
            .biofilter_maturity(0.03, 1.0)
            .plant_biomass_g(1.0, 400.0),
    );
    run.finish()?;

    assert!(
        metrics.cycling_completion_hours.is_some(),
        "{label}: stocked baseline should cross the shared cycle-timeline maturity floor within {STOCKED_DURATION_HOURS}h; metrics={metrics:?}"
    );

    Ok(metrics)
}

fn assert_pair_within_fraction(label: &str, lhs: f64, rhs: f64, tolerance: f64) {
    let scale = lhs.abs().max(rhs.abs()).max(1e-9);
    let fraction = (lhs - rhs).abs() / scale;
    assert!(
        fraction <= tolerance,
        "{label}: values should stay within {:.0}% (lhs={lhs:.4}, rhs={rhs:.4}, fraction={:.2}%)",
        tolerance * 100.0,
        fraction * 100.0
    );
}

fn assert_group_within_fraction(label: &str, values: &[f64], tolerance: f64) {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let scale = max.abs().max(1e-9);
    let fraction = (max - min).abs() / scale;
    assert!(
        fraction <= tolerance,
        "{label}: values should stay within {:.0}% across the group (values={values:?}, fraction={:.2}%)",
        tolerance * 100.0,
        fraction * 100.0
    );
}

#[test]
fn geometry_1x_vs_2x_stocked_metrics_stay_within_20_percent(
) -> Result<(), Box<dyn std::error::Error>> {
    let metrics_1x = collect_stocked_metrics(42, 1.0, "geom_scale_1x", None)?;
    let metrics_2x = collect_stocked_metrics(42, 2.0, "geom_scale_2x", None)?;

    assert_pair_within_fraction(
        "1x vs 2x cycle timeline",
        metrics_1x.cycling_completion_hours.unwrap() as f64,
        metrics_2x.cycling_completion_hours.unwrap() as f64,
        PARITY_TOLERANCE,
    );
    assert_pair_within_fraction(
        "1x vs 2x peak TAN",
        metrics_1x.peak_tan_mg_n_per_l,
        metrics_2x.peak_tan_mg_n_per_l,
        CONCENTRATION_PARITY_TOLERANCE,
    );
    assert_pair_within_fraction(
        "1x vs 2x DO range",
        metrics_1x.do_range_mg_l(),
        metrics_2x.do_range_mg_l(),
        CONCENTRATION_PARITY_TOLERANCE,
    );
    assert_pair_within_fraction(
        "1x vs 2x final temperature",
        metrics_1x.final_temperature_c,
        metrics_2x.final_temperature_c,
        PARITY_TOLERANCE,
    );

    Ok(())
}

#[test]
fn mismatched_filter_shows_worse_cycling_than_auto_scaled() -> Result<(), Box<dyn std::error::Error>>
{
    let auto = collect_stocked_metrics(99, 2.0, "mismatch_auto", None)?;
    let tiny = collect_stocked_metrics(99, 2.0, "mismatch_tiny_filter", Some(200.0))?;

    assert!(
        tiny.peak_nitrite_mg_n_per_l > auto.peak_nitrite_mg_n_per_l * 1.5,
        "tiny filter should show a materially higher nitrite peak than the auto-scaled setup: auto={auto:?}, tiny={tiny:?}"
    );
    assert!(
        tiny.final_nitrite_mg_n_per_l > auto.final_nitrite_mg_n_per_l * 1.5,
        "tiny filter should retain materially more nitrite by the end of the run: auto={auto:?}, tiny={tiny:?}"
    );

    Ok(())
}

#[test]
fn nano_vs_standard_vs_large_stocked_runs_scale_within_20_percent(
) -> Result<(), Box<dyn std::error::Error>> {
    let configs = [
        ("small_20l", (20.0_f64 / 57.6).cbrt()),
        ("standard_60l", 1.0),
        ("large_200l", (200.0_f64 / 57.6).cbrt()),
    ];

    let mut metrics = Vec::new();
    for (label, size_scale) in configs {
        metrics.push(collect_stocked_metrics(77, size_scale, label, None)?);
    }

    let cycle_hours = metrics
        .iter()
        .map(|metric| metric.cycling_completion_hours.unwrap() as f64)
        .collect::<Vec<_>>();
    let peak_tan = metrics
        .iter()
        .map(|metric| metric.peak_tan_mg_n_per_l)
        .collect::<Vec<_>>();
    let do_ranges = metrics
        .iter()
        .map(ScalingMetrics::do_range_mg_l)
        .collect::<Vec<_>>();

    assert_group_within_fraction(
        "20L/60L/200L cycle timeline",
        &cycle_hours,
        PARITY_TOLERANCE,
    );
    assert_group_within_fraction(
        "20L/60L/200L peak TAN",
        &peak_tan,
        CONCENTRATION_PARITY_TOLERANCE,
    );
    assert_group_within_fraction(
        "20L/60L/200L DO range",
        &do_ranges,
        CONCENTRATION_PARITY_TOLERANCE,
    );

    for metric in metrics {
        assert!(
            metric.cycling_completion_hours.is_some(),
            "{}: stocked run should report a cycle-timeline crossing, metrics={metric:?}",
            metric.label
        );
        assert!(
            metric.adult_shrimp_count > 0
                && metric.net_volume_l > 0.0
                && metric.gross_volume_l > 0.0,
            "{}: metrics should capture a real stocked run, metrics={metric:?}",
            metric.label
        );
        assert!(
            metric.final_biofilter_maturity_index > 0.0,
            "{}: final maturity should remain finite and positive, metrics={metric:?}",
            metric.label
        );
    }

    Ok(())
}
