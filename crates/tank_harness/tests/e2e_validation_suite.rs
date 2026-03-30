//! Literature-backed validation suite (tanksim-6e5.7.3).
//!
//! Run all probes:   `cargo test --test e2e_validation_suite`
//! Verbose traces:   `TANK_E2E_VERBOSE=1 cargo test --test e2e_validation_suite -- --nocapture`
//! Summary only:     `cargo test --test e2e_validation_suite validation_suite_summary -- --nocapture`

#[test]
fn vs01_cycling_timeline() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs01_cycling_timeline()
}

#[test]
fn vs02_aeration_effects() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs02_aeration_effects()
}

#[test]
fn vs03_day_night_ph_swing() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs03_day_night_ph_swing()
}

#[test]
fn vs04_source_water_differentiation() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs04_source_water_differentiation()
}

#[test]
fn vs05_shrimp_breeding_thermal_window() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs05_shrimp_breeding_thermal_window()
}

#[test]
fn vs06_algae_plant_competition() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs06_algae_plant_competition()
}

#[test]
fn vs07_nitrate_removal_denitrification() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs07_nitrate_removal_denitrification()
}

#[test]
fn vs08_stocking_density_crash() -> Result<(), Box<dyn std::error::Error>> {
    tank_harness::validation_suite::vs08_stocking_density_crash()
}

#[test]
fn validation_suite_summary() {
    tank_harness::validation_suite::validation_suite_summary();
}
