use tank_core::{
    systems::nitrogen_cycle::{compute_biofilter_carrying_capacity, step_nitrogen_cycle},
    Engine, HabitatEntry, HabitatKind, PlayerAction, SimSeed, SimulationEngine, TankState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal TankState with the given filter media area and flow,
/// refresh the habitat registry, and return it ready for nitrogen cycle use.
fn state_with_filter(media_area_cm2: f64, flow_lph: f64) -> TankState {
    let mut state = TankState::new(SimSeed(5500));
    state.hardware.filter.enabled = true;
    state.hardware.filter.media_area_cm2 = media_area_cm2;
    state.hardware.filter.flow_lph = flow_lph;
    state.hardware.filter.cleanliness_index = 1.0;
    state.filter_state.clogging_index = 0.0;
    state.filter_state.biofilter_maturity_index = 1.0;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.5;
    state.water.temperature_c = 26.0;
    state.refresh_habitat_registry();
    state
}

// ---------------------------------------------------------------------------
// 1. Capacity scales with media area
// ---------------------------------------------------------------------------

#[test]
fn test_biofilter_capacity_scales_with_media_area() -> Result<(), Box<dyn std::error::Error>> {
    let state_small = state_with_filter(2000.0, 200.0);
    let state_large = state_with_filter(4000.0, 200.0);

    let density = state_small.process_params.nitrifier_base_density_g_per_cm2;
    let cap_small = compute_biofilter_carrying_capacity(&state_small.habitat_registry, density);
    let cap_large = compute_biofilter_carrying_capacity(&state_large.habitat_registry, density);

    // Doubling the filter media area should significantly increase capacity.
    // Glass walls and substrate also contribute a fixed base, so the ratio
    // won't be exactly 2x, but larger filter must produce meaningfully higher
    // capacity (at least 30% more).
    let ratio = cap_large / cap_small;
    assert!(
        ratio > 1.3,
        "2x media area should yield at least 30% more capacity, got ratio {ratio:.3} \
         (small={cap_small:.4} g, large={cap_large:.4} g)"
    );

    // The increase should be proportional to the media area increase within
    // the filter-media component (verify the capacity goes up, not just noise).
    assert!(
        cap_large - cap_small > 0.1,
        "Capacity difference should be substantial: \
         small={cap_small:.4} g, large={cap_large:.4} g"
    );

    // Also verify via simulation: run both for many hours and check biomass ceiling.
    let mut s1 = state_small;
    let mut s2 = state_large;
    // Seed generous TAN and resources
    s1.water.ammonia_total_mg_n_total = 200.0;
    s1.water.dissolved_oxygen_mg_total = 5000.0;
    s1.water.alkalinity_meq_total = 5000.0;
    s1.water.dissolved_inorganic_carbon_mg_c_total = 5000.0;
    s2.water.ammonia_total_mg_n_total = 200.0;
    s2.water.dissolved_oxygen_mg_total = 5000.0;
    s2.water.alkalinity_meq_total = 5000.0;
    s2.water.dissolved_inorganic_carbon_mg_c_total = 5000.0;

    for _ in 0..500 {
        step_nitrogen_cycle(&mut s1);
        step_nitrogen_cycle(&mut s2);
    }

    let biomass_small = s1.microbe.ammonia_oxidizer_biomass_g
        + s1.microbe.nitrite_oxidizer_biomass_g
        + s1.microbe.comammox_biomass_g;
    let biomass_large = s2.microbe.ammonia_oxidizer_biomass_g
        + s2.microbe.nitrite_oxidizer_biomass_g
        + s2.microbe.comammox_biomass_g;

    assert!(
        biomass_large > biomass_small,
        "Larger filter should achieve higher nitrifier biomass: \
         small={biomass_small:.4} g, large={biomass_large:.4} g"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Capacity scales with flow rate
// ---------------------------------------------------------------------------

#[test]
fn test_biofilter_capacity_scales_with_flow() -> Result<(), Box<dyn std::error::Error>> {
    let state_low_flow = state_with_filter(2000.0, 100.0);
    let state_high_flow = state_with_filter(2000.0, 200.0);

    let density = state_low_flow
        .process_params
        .nitrifier_base_density_g_per_cm2;
    let cap_low = compute_biofilter_carrying_capacity(&state_low_flow.habitat_registry, density);
    let cap_high = compute_biofilter_carrying_capacity(&state_high_flow.habitat_registry, density);

    // Higher flow should yield higher capacity via flow_exposure modifier.
    assert!(
        cap_high > cap_low,
        "Higher flow should increase capacity: low_flow={cap_low:.4} g, high_flow={cap_high:.4} g"
    );

    // The flow × area interaction means it's not just area alone.
    let ratio = cap_high / cap_low;
    assert!(
        ratio > 1.01,
        "Flow increase should produce a measurable capacity increase, got ratio {ratio:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. O2 limitation reduces capacity
// ---------------------------------------------------------------------------

#[test]
fn test_biofilter_o2_limitation() -> Result<(), Box<dyn std::error::Error>> {
    // Build two identical tanks, but one has no aeration (lower O2 exposure).
    let mut state_aerated = state_with_filter(2000.0, 200.0);
    state_aerated.hardware.aeration.enabled = true;
    state_aerated.hardware.aeration.intensity = 0.8;
    state_aerated.refresh_habitat_registry();

    let mut state_no_aeration = state_with_filter(2000.0, 200.0);
    state_no_aeration.hardware.aeration.enabled = false;
    state_no_aeration.hardware.aeration.intensity = 0.0;
    state_no_aeration.refresh_habitat_registry();

    let density = state_aerated
        .process_params
        .nitrifier_base_density_g_per_cm2;
    let cap_aerated = compute_biofilter_carrying_capacity(&state_aerated.habitat_registry, density);
    let cap_no_aeration =
        compute_biofilter_carrying_capacity(&state_no_aeration.habitat_registry, density);

    // Aerated tank should have higher capacity due to higher oxygen exposure.
    assert!(
        cap_aerated > cap_no_aeration,
        "Aerated tank should have higher capacity: \
         aerated={cap_aerated:.4} g, no_aeration={cap_no_aeration:.4} g"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Zero media yields near-zero capacity
// ---------------------------------------------------------------------------

#[test]
fn test_biofilter_zero_media_zero_capacity() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = TankState::new(SimSeed(5504));
    state.hardware.filter.enabled = false;
    state.hardware.filter.media_area_cm2 = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.geometry.hardscape_area_cm2 = 0.0;
    state.refresh_habitat_registry();

    let density = state.process_params.nitrifier_base_density_g_per_cm2;
    let cap = compute_biofilter_carrying_capacity(&state.habitat_registry, density);

    // With no filter media and no substrate/hardscape, capacity should be
    // very low (only glass walls contribute via their colonizable area).
    // This should be dramatically less than a tank with a proper filter
    // (default 2000 cm² yields ~0.5 g+).
    assert!(
        cap < 0.15,
        "Tank with no filter/substrate/hardscape should have very low capacity, got {cap:.4} g"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Habitat modifiers are correctly applied
// ---------------------------------------------------------------------------

#[test]
fn test_habitat_modifiers_applied_correctly() -> Result<(), Box<dyn std::error::Error>> {
    let density = 1.0; // Use 1.0 for easy verification

    // Hand-craft habitat entries to verify the formula directly.
    let registry = vec![
        HabitatEntry {
            kind: HabitatKind::FilterMedia,
            colonizable_area_cm2: 100.0,
            flow_exposure: 0.8,
            oxygen_exposure: 0.6,
            light_exposure: 0.0,
        },
        HabitatEntry {
            kind: HabitatKind::GlassHardscape,
            colonizable_area_cm2: 50.0,
            flow_exposure: 0.4,
            oxygen_exposure: 0.5,
            light_exposure: 0.3,
        },
    ];

    let cap = compute_biofilter_carrying_capacity(&registry, density);
    // Expected: (100 * 0.8 * 0.6) + (50 * 0.4 * 0.5) = 48.0 + 10.0 = 58.0
    let expected = 58.0;
    assert!(
        (cap - expected).abs() < 0.001,
        "Expected capacity {expected}, got {cap}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Integration: cycling speed varies with biofilter capacity
// ---------------------------------------------------------------------------

#[test]
fn test_cycling_speed_varies_with_biofilter_capacity() -> Result<(), Box<dyn std::error::Error>> {
    // Nano tank: small sponge filter (500 cm² media, low flow)
    let mut nano = TankState::new(SimSeed(5510));
    nano.geometry.length_cm = 30.0;
    nano.geometry.width_cm = 20.0;
    nano.geometry.height_cm = 20.0;
    nano.geometry.fill_height_cm = 18.0;
    nano.hardware.filter.enabled = true;
    nano.hardware.filter.media_area_cm2 = 500.0;
    nano.hardware.filter.flow_lph = 80.0;
    nano.hardware.filter.cleanliness_index = 1.0;
    nano.filter_state.clogging_index = 0.0;
    nano.filter_state.biofilter_maturity_index = 0.05;
    nano.hardware.aeration.enabled = true;
    nano.hardware.aeration.intensity = 0.3;
    nano.water = tank_core::WaterState::default_for_volume_l(nano.water_volume_l());
    nano.water.temperature_c = 25.0;
    nano.microbe.ammonia_oxidizer_biomass_g = 0.005;
    nano.microbe.nitrite_oxidizer_biomass_g = 0.005;
    nano.microbe.comammox_biomass_g = 0.001;
    nano.refresh_habitat_registry();

    // Medium tank: canister filter (4000 cm² media, high flow)
    let mut medium = TankState::new(SimSeed(5510));
    medium.geometry.length_cm = 60.0;
    medium.geometry.width_cm = 30.0;
    medium.geometry.height_cm = 36.0;
    medium.geometry.fill_height_cm = 32.0;
    medium.hardware.filter.enabled = true;
    medium.hardware.filter.media_area_cm2 = 4000.0;
    medium.hardware.filter.flow_lph = 400.0;
    medium.hardware.filter.cleanliness_index = 1.0;
    medium.filter_state.clogging_index = 0.0;
    medium.filter_state.biofilter_maturity_index = 0.05;
    medium.hardware.aeration.enabled = true;
    medium.hardware.aeration.intensity = 0.3;
    medium.water = tank_core::WaterState::default_for_volume_l(medium.water_volume_l());
    medium.water.temperature_c = 25.0;
    medium.microbe.ammonia_oxidizer_biomass_g = 0.005;
    medium.microbe.nitrite_oxidizer_biomass_g = 0.005;
    medium.microbe.comammox_biomass_g = 0.001;
    medium.refresh_habitat_registry();

    let mut nano_engine = Engine::from_parts(nano, vec![]);
    let mut medium_engine = Engine::from_parts(medium, vec![]);

    let days = 45;
    let nano_feed = 0.15; // lighter feed for nano
    let medium_feed = 0.30; // heavier feed for medium (proportional to volume)

    let mut nano_cycled_hour: Option<u32> = None;
    let mut medium_cycled_hour: Option<u32> = None;

    for day in 0..days {
        nano_engine.apply_action(PlayerAction::Feed { grams: nano_feed })?;
        medium_engine.apply_action(PlayerAction::Feed { grams: medium_feed })?;

        for h in 0..24 {
            nano_engine.step_hours(1)?;
            medium_engine.step_hours(1)?;

            let hour = day * 24 + h + 1;

            // "Cycled" = TAN and nitrite both below 0.5 mg/L after initial spike
            if nano_cycled_hour.is_none() && hour > 72 {
                let tan = nano_engine.full_state().tan_mg_n_per_l();
                let no2 = nano_engine.full_state().nitrite_mg_n_per_l();
                if tan < 0.5 && no2 < 0.5 {
                    nano_cycled_hour = Some(hour);
                }
            }
            if medium_cycled_hour.is_none() && hour > 72 {
                let tan = medium_engine.full_state().tan_mg_n_per_l();
                let no2 = medium_engine.full_state().nitrite_mg_n_per_l();
                if tan < 0.5 && no2 < 0.5 {
                    medium_cycled_hour = Some(hour);
                }
            }
        }
    }

    // The medium tank with 8x more filter media should cycle faster
    // (or at least not slower) than the nano tank, despite higher feed load.
    match (nano_cycled_hour, medium_cycled_hour) {
        (Some(nano_h), Some(medium_h)) => {
            assert!(
                medium_h <= nano_h,
                "Medium tank with larger filter should cycle at least as fast: \
                 nano={nano_h}h, medium={medium_h}h"
            );
        }
        (None, Some(_medium_h)) => {
            // Medium cycled but nano didn't — expected outcome, pass.
        }
        (Some(nano_h), None) => {
            panic!(
                "Nano cycled at {nano_h}h but medium never cycled — \
                 larger filter should not be slower"
            );
        }
        (None, None) => {
            // Neither cycled in 45 days. Compare final nitrifier biomass instead:
            // medium should have more nitrifier biomass due to higher capacity.
            let nano_biomass = {
                let s = nano_engine.full_state();
                s.microbe.ammonia_oxidizer_biomass_g
                    + s.microbe.nitrite_oxidizer_biomass_g
                    + s.microbe.comammox_biomass_g
            };
            let medium_biomass = {
                let s = medium_engine.full_state();
                s.microbe.ammonia_oxidizer_biomass_g
                    + s.microbe.nitrite_oxidizer_biomass_g
                    + s.microbe.comammox_biomass_g
            };
            assert!(
                medium_biomass > nano_biomass,
                "Medium tank should have higher nitrifier biomass: \
                 nano={nano_biomass:.4} g, medium={medium_biomass:.4} g"
            );
        }
    }

    Ok(())
}
