//! Tests verifying that source-water presets carry carbonate-relevant
//! inputs and produce meaningfully differentiated pH behavior.

use tank_core::{
    Engine, PlayerAction, SimError, SimSeed, SimulationEngine, SourceWaterProfile, TankState,
    WaterState,
};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_source_profile(id: &str) -> SourceWaterProfile {
    let preset = tank_data::load_source_water(id)
        .unwrap_or_else(|e| panic!("failed to load source water preset '{id}': {e}"));
    SourceWaterProfile {
        temperature_c: preset.temperature_c,
        ammonia_mg_n_per_l: preset.ammonia_mg_n_per_l,
        nitrite_mg_n_per_l: preset.nitrite_mg_n_per_l,
        nitrate_mg_n_per_l: preset.nitrate_mg_n_per_l,
        phosphate_mg_p_per_l: preset.phosphate_mg_p_per_l,
        dic_mg_c_per_l: preset.dic_mg_c_per_l,
        doc_mg_c_per_l: preset.doc_mg_c_per_l,
        don_mg_n_per_l: preset.don_mg_n_per_l,
        alkalinity_meq_per_l: preset.alkalinity_meq_per_l,
        calcium_mg_per_l: preset.calcium_mg_per_l,
        magnesium_mg_per_l: preset.magnesium_mg_per_l,
        sodium_mg_per_l: preset.sodium_mg_per_l,
        potassium_mg_per_l: preset.potassium_mg_per_l,
        bicarbonate_mg_per_l: preset.bicarbonate_mg_per_l,
        chloride_mg_per_l: preset.chloride_mg_per_l,
        sulfate_mg_per_l: preset.sulfate_mg_per_l,
    }
}

/// Build a minimal tank state with a given source water profile and volume.
fn tank_with_source(profile: &SourceWaterProfile, volume_l: f64, seed: SimSeed) -> TankState {
    let water = WaterState::from_source_profile_for_volume_l(profile, volume_l);
    let mut state = TankState::new(seed);
    state.water = water;
    // Populate source water catalog so water changes work.
    for sw_id in tank_data::source_water_ids() {
        state
            .source_water_catalog
            .insert(sw_id.to_string(), load_source_profile(sw_id));
    }
    state
}

// ---------------------------------------------------------------------------
// 1. test_source_water_carries_dic
// ---------------------------------------------------------------------------

#[test]
fn test_source_water_carries_dic() -> Result<(), Box<dyn std::error::Error>> {
    let ids = ["soft_acidic", "moderate", "hard_shrimp", "ro_like"];
    for id in ids {
        let profile = load_source_profile(id);
        assert!(
            profile.dic_mg_c_per_l > 0.0,
            "source water '{id}' should have positive DIC, got {}",
            profile.dic_mg_c_per_l
        );
        assert!(
            profile.dic_mg_c_per_l.is_finite(),
            "source water '{id}' DIC must be finite"
        );
    }

    // hard_shrimp should have the highest DIC
    let hard = load_source_profile("hard_shrimp");
    let soft = load_source_profile("soft_acidic");
    let ro = load_source_profile("ro_like");
    assert!(
        hard.dic_mg_c_per_l > soft.dic_mg_c_per_l,
        "hard_shrimp DIC ({}) should exceed soft_acidic DIC ({})",
        hard.dic_mg_c_per_l,
        soft.dic_mg_c_per_l
    );
    assert!(
        soft.dic_mg_c_per_l > ro.dic_mg_c_per_l,
        "soft_acidic DIC ({}) should exceed ro_like DIC ({})",
        soft.dic_mg_c_per_l,
        ro.dic_mg_c_per_l
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. test_source_water_carries_alkalinity
// ---------------------------------------------------------------------------

#[test]
fn test_source_water_carries_alkalinity() -> Result<(), Box<dyn std::error::Error>> {
    let hard = load_source_profile("hard_shrimp");
    let moderate = load_source_profile("moderate");
    let soft = load_source_profile("soft_acidic");
    let ro = load_source_profile("ro_like");

    // All should have non-negative alkalinity
    for (id, alk) in [
        ("hard_shrimp", hard.alkalinity_meq_per_l),
        ("moderate", moderate.alkalinity_meq_per_l),
        ("soft_acidic", soft.alkalinity_meq_per_l),
        ("ro_like", ro.alkalinity_meq_per_l),
    ] {
        assert!(
            alk >= 0.0 && alk.is_finite(),
            "source water '{id}' alkalinity must be finite and non-negative, got {alk}"
        );
    }

    // Ordering: hard > moderate > soft > ro
    assert!(
        hard.alkalinity_meq_per_l > moderate.alkalinity_meq_per_l,
        "hard_shrimp alkalinity ({}) should exceed moderate ({})",
        hard.alkalinity_meq_per_l,
        moderate.alkalinity_meq_per_l
    );
    assert!(
        moderate.alkalinity_meq_per_l > soft.alkalinity_meq_per_l,
        "moderate alkalinity ({}) should exceed soft_acidic ({})",
        moderate.alkalinity_meq_per_l,
        soft.alkalinity_meq_per_l
    );
    assert!(
        soft.alkalinity_meq_per_l > ro.alkalinity_meq_per_l,
        "soft_acidic alkalinity ({}) should exceed ro_like ({})",
        soft.alkalinity_meq_per_l,
        ro.alkalinity_meq_per_l
    );

    // KH approximate checks via the meq -> °dKH conversion (1 meq/L ≈ 2.8 °dKH)
    let kh_hard = hard.alkalinity_meq_per_l * 2.8;
    let kh_ro = ro.alkalinity_meq_per_l * 2.8;
    assert!(
        kh_hard > 7.0 && kh_hard < 10.0,
        "hard_shrimp KH should be ~8, got {kh_hard:.1}"
    );
    assert!(kh_ro < 0.5, "ro_like KH should be ~0.1, got {kh_ro:.2}");

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. test_water_change_brings_source_dic
// ---------------------------------------------------------------------------

#[test]
fn test_water_change_brings_source_dic() -> Result<(), SimError> {
    let hard = load_source_profile("hard_shrimp");
    let volume_l = 20.0;

    // Start with low-DIC water (ro_like-derived)
    let ro = load_source_profile("ro_like");
    let state = tank_with_source(&ro, volume_l, SimSeed(5000));
    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;

    // 50% water change with hard_shrimp source
    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "hard_shrimp".to_string(),
    })?;
    engine.step_hours(1)?;

    let dic_after = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;

    // DIC should have increased toward hard_shrimp source values.
    // After 50% WC: new_DIC ≈ 0.5 * old_DIC + 0.5 * hard_source_DIC * volume
    let expected_dic_approx = 0.5 * dic_before + 0.5 * hard.dic_mg_c_per_l * volume_l;
    // Allow some tolerance for the 1-hour step biological activity
    let tolerance = 0.15; // 15%
    let ratio = dic_after / expected_dic_approx;
    assert!(
        (ratio - 1.0).abs() < tolerance,
        "DIC after 50% WC with hard_shrimp should be near {expected_dic_approx:.1}, got {dic_after:.1} (ratio {ratio:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. test_water_change_brings_source_alkalinity
// ---------------------------------------------------------------------------

#[test]
fn test_water_change_brings_source_alkalinity() -> Result<(), SimError> {
    let hard = load_source_profile("hard_shrimp");
    let volume_l = 20.0;

    // Start with low-alkalinity water (ro_like-derived)
    let ro = load_source_profile("ro_like");
    let state = tank_with_source(&ro, volume_l, SimSeed(5001));
    let alk_before = state.water.alkalinity_meq_total;

    // 50% water change with hard_shrimp source
    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "hard_shrimp".to_string(),
    })?;
    engine.step_hours(1)?;

    let alk_after = engine.full_state().water.alkalinity_meq_total;

    // Alkalinity should shift toward hard_shrimp source values.
    let expected_alk_approx = 0.5 * alk_before + 0.5 * hard.alkalinity_meq_per_l * volume_l;
    let tolerance = 0.10; // 10%
    let ratio = alk_after / expected_alk_approx;
    assert!(
        (ratio - 1.0).abs() < tolerance,
        "Alkalinity after 50% WC with hard_shrimp should be near {expected_alk_approx:.3}, got {alk_after:.3} (ratio {ratio:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. test_differentiated_source_waters_produce_different_ph
// ---------------------------------------------------------------------------

#[test]
fn test_differentiated_source_waters_produce_different_ph() -> Result<(), SimError> {
    let soft = load_source_profile("soft_acidic");
    let hard = load_source_profile("hard_shrimp");
    let volume_l = 30.0;

    // Create two tanks from different source waters and let them equilibrate
    let mut soft_state = tank_with_source(&soft, volume_l, SimSeed(5010));
    let mut hard_state = tank_with_source(&hard, volume_l, SimSeed(5010));

    // Suppress biological activity AND gas exchange so we see equilibrium
    // pH from the DIC/alkalinity ratio alone, without atmospheric CO2
    // driving both tanks toward the same alkaline ceiling.
    for state in [&mut soft_state, &mut hard_state] {
        state.plant_guilds.clear();
        state.algae.set_periphyton_total(0.0);
        state.algae.suspended_biomass_g = 0.0;
        state.microbe.ammonia_oxidizer_biomass_g = 0.0;
        state.microbe.nitrite_oxidizer_biomass_g = 0.0;
        state.microbe.comammox_biomass_g = 0.0;
        state.microbe.set_decomposer_total(0.0);
        state.animal.adult.count = 0;
        state.animal.juvenile.count = 0;
        // Suppress CO2 gas exchange so DIC stays at source-water levels.
        state.process_params.reaeration_kla_base = 0.0;
        state.process_params.aeration_kla_boost = 0.0;
    }

    let mut soft_engine = Engine::from_parts(soft_state, vec![]);
    let mut hard_engine = Engine::from_parts(hard_state, vec![]);

    // Run for 100 hours to reach equilibrium
    soft_engine.step_hours(100)?;
    hard_engine.step_hours(100)?;

    let soft_ph = soft_engine.full_state().water.ph;
    let hard_ph = hard_engine.full_state().water.ph;

    let ph_diff = hard_ph - soft_ph;
    assert!(
        ph_diff > 0.5,
        "hard_shrimp pH ({hard_ph:.3}) minus soft_acidic pH ({soft_ph:.3}) = {ph_diff:.3}, \
         should be > 0.5 for meaningful differentiation"
    );

    // Sanity: both pH values should be in their expected ranges
    assert!(
        soft_ph >= 5.5 && soft_ph <= 7.5,
        "soft_acidic pH {soft_ph:.3} should be in 5.5-7.5 range"
    );
    assert!(
        hard_ph >= 7.0 && hard_ph <= 8.5,
        "hard_shrimp pH {hard_ph:.3} should be in 7.0-8.5 range"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. test_ro_water_minimal_buffering
// ---------------------------------------------------------------------------

#[test]
fn test_ro_water_minimal_buffering() -> Result<(), SimError> {
    let moderate = load_source_profile("moderate");
    let ro = load_source_profile("ro_like");
    let volume_l = 20.0;

    // Start with a moderate-buffered tank
    let state = tank_with_source(&moderate, volume_l, SimSeed(5020));
    let alk_before = state.water.alkalinity_meq_total;

    assert!(
        alk_before > 0.5,
        "moderate-filled tank should have substantial alkalinity"
    );

    // 50% water change with RO water should reduce alkalinity significantly
    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "ro_like".to_string(),
    })?;
    engine.step_hours(1)?;

    let alk_after = engine.full_state().water.alkalinity_meq_total;

    // RO water adds nearly zero alkalinity, so after 50% WC, alkalinity
    // should be close to 50% of the original (plus the tiny RO contribution).
    let expected_fraction = 0.5;
    let actual_fraction = alk_after / alk_before;
    let tolerance = 0.10; // 10% tolerance for 1 hour of biological activity
    assert!(
        (actual_fraction - expected_fraction).abs() < tolerance,
        "alkalinity should decrease to ~50% after 50% RO water change, \
         got {actual_fraction:.3} (before={alk_before:.3}, after={alk_after:.3})"
    );

    // Verify the RO source itself has very low alkalinity
    assert!(
        ro.alkalinity_meq_per_l < 0.05,
        "RO-like water should have near-zero alkalinity, got {}",
        ro.alkalinity_meq_per_l
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 7. test_source_water_validation
// ---------------------------------------------------------------------------

#[test]
fn test_source_water_validation() -> Result<(), Box<dyn std::error::Error>> {
    // Negative DIC should be rejected
    let mut bad = SourceWaterProfile::zero();
    bad.dic_mg_c_per_l = -1.0;
    let err = bad.validate("negative_dic");
    assert!(err.is_err(), "negative DIC should be rejected");

    // NaN DIC should be rejected
    let mut bad_nan = SourceWaterProfile::zero();
    bad_nan.dic_mg_c_per_l = f64::NAN;
    let err = bad_nan.validate("nan_dic");
    assert!(err.is_err(), "NaN DIC should be rejected");

    // Negative alkalinity should be rejected
    let mut bad_alk = SourceWaterProfile::zero();
    bad_alk.dic_mg_c_per_l = 10.0; // need positive DIC so carbonate check runs
    bad_alk.alkalinity_meq_per_l = -0.5;
    let err = bad_alk.validate("negative_alk");
    assert!(err.is_err(), "negative alkalinity should be rejected");

    // NaN alkalinity should be rejected
    let mut bad_alk_nan = SourceWaterProfile::zero();
    bad_alk_nan.dic_mg_c_per_l = 10.0;
    bad_alk_nan.alkalinity_meq_per_l = f64::NAN;
    let err = bad_alk_nan.validate("nan_alk");
    assert!(err.is_err(), "NaN alkalinity should be rejected");

    // All shipped presets should pass validation
    for id in tank_data::source_water_ids() {
        let profile = load_source_profile(id);
        profile
            .validate(id)
            .unwrap_or_else(|e| panic!("shipped preset '{id}' failed validation: {e}"));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 8. test_source_water_ph_story (integration)
// ---------------------------------------------------------------------------

#[test]
fn test_source_water_ph_story() -> Result<(), SimError> {
    let seed = SimSeed(5050);

    // Run nano_cycle scenario twice: once with soft_acidic, once with hard_shrimp
    let soft_overrides = StartupOverrides {
        source_water_profile_id: Some("soft_acidic".to_string()),
        substrate_preset: Some(StartupSubstratePreset::InertSand),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours8),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(false),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };

    let hard_overrides = StartupOverrides {
        source_water_profile_id: Some("hard_shrimp".to_string()),
        ..soft_overrides.clone()
    };

    let soft_state =
        tank_scenarios::seeded_state_with_full_overrides(seed, "nano_cycle", soft_overrides)
            .expect("failed to materialize soft_acidic nano_cycle");

    let hard_state =
        tank_scenarios::seeded_state_with_full_overrides(seed, "nano_cycle", hard_overrides)
            .expect("failed to materialize hard_shrimp nano_cycle");

    let soft_ph_initial = soft_state.water.ph;
    let hard_ph_initial = hard_state.water.ph;

    let mut soft_engine = Engine::from_parts(soft_state, vec![]);
    let mut hard_engine = Engine::from_parts(hard_state, vec![]);

    // Feed both tanks identically for 500 hours (~21 days) to drive nitrification
    let feed_g = 0.05;
    for hour in 0..500 {
        if hour % 24 == 8 {
            soft_engine
                .apply_action(PlayerAction::Feed { grams: feed_g })
                .expect("soft feed failed");
            hard_engine
                .apply_action(PlayerAction::Feed { grams: feed_g })
                .expect("hard feed failed");
        }
        soft_engine.step_hours(1)?;
        hard_engine.step_hours(1)?;
    }

    let soft_ph_final = soft_engine.full_state().water.ph;
    let hard_ph_final = hard_engine.full_state().water.ph;

    // Soft water tank should have lower pH than hard water tank
    assert!(
        soft_ph_final < hard_ph_final,
        "soft_acidic tank pH ({soft_ph_final:.3}) should be lower than \
         hard_shrimp tank pH ({hard_ph_final:.3}) after 500 hours"
    );

    // Soft water should show faster pH decline (less buffering)
    let soft_ph_drop = soft_ph_initial - soft_ph_final;
    let hard_ph_drop = hard_ph_initial - hard_ph_final;
    assert!(
        soft_ph_drop > hard_ph_drop || soft_ph_final < hard_ph_final - 0.3,
        "soft water should show greater pH decline or significantly lower pH: \
         soft drop={soft_ph_drop:.3} (from {soft_ph_initial:.3} to {soft_ph_final:.3}), \
         hard drop={hard_ph_drop:.3} (from {hard_ph_initial:.3} to {hard_ph_final:.3})"
    );

    // Hard water tank should maintain more stable pH (well-buffered)
    assert!(
        hard_ph_final >= 7.0,
        "hard_shrimp tank pH ({hard_ph_final:.3}) should remain >= 7.0 \
         after 500 hours with moderate feeding"
    );

    // Envelope bounds
    assert!(
        soft_ph_final >= 5.5 && soft_ph_final <= 8.5,
        "soft_acidic pH {soft_ph_final:.3} out of solver bounds"
    );
    assert!(
        hard_ph_final >= 5.5 && hard_ph_final <= 8.5,
        "hard_shrimp pH {hard_ph_final:.3} out of solver bounds"
    );

    Ok(())
}
