use proptest::prelude::*;
use tank_core::{
    SimSeed, SourceWaterProfile, SubstrateKind, SubstrateLayerState, TankGeometry, TankState,
    WaterState,
};

fn helper_state() -> TankState {
    let mut state = TankState::new(SimSeed(12_345));
    state.geometry = TankGeometry {
        length_cm: 50.0,
        width_cm: 20.0,
        height_cm: 15.0,
        fill_height_cm: 12.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    state.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::InertSand,
        depth_cm: 2.0,
        nutrient_store_mg_n_total: 0.0,
        nutrient_store_mg_p_total: 0.0,
        cation_exchange_capacity_index: 0.1,
        detritus_trapping_index: 0.3,
        colonizable_area_cm2: state.geometry.footprint_area_cm2(),
        low_oxygen_tendency_index: 0.2,
        grazing_surface_index: 0.4,
    }];
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn expected_gh_d(calcium_mg_per_l: f64, magnesium_mg_per_l: f64) -> f64 {
    ((2.497 * calcium_mg_per_l) + (4.118 * magnesium_mg_per_l)) / 17.848
}

fn expected_kh_d(alkalinity_meq_per_l: f64) -> f64 {
    (alkalinity_meq_per_l * 50.0) / 17.848
}

#[test]
fn canonical_helpers_use_net_water_volume() {
    let mut state = helper_state();
    assert_close(state.geometry.water_volume_l(), 12.0);
    assert_close(state.substrate_volume_l(), 2.0);
    assert_close(state.water_volume_l(), 10.0);

    state.water.ammonia_total_mg_n_total = 15.0;
    state.water.nitrite_mg_n_total = 7.0;
    state.water.nitrate_mg_n_total = 30.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 12.0;
    state.water.dissolved_organic_carbon_mg_c_total = 50.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 120.0;
    state.water.dissolved_oxygen_mg_total = 80.0;
    state.water.phosphate_mg_p_total = 9.0;
    state.water.alkalinity_meq_total = 25.0;
    state.water.calcium_mg_total = 200.0;
    state.water.magnesium_mg_total = 50.0;
    state.water.sodium_mg_total = 100.0;
    state.water.potassium_mg_total = 30.0;
    state.water.bicarbonate_mg_total = 700.0;
    state.water.chloride_mg_total = 120.0;
    state.water.sulfate_mg_total = 80.0;

    assert_close(state.tan_mg_n_per_l(), 1.5);
    assert_close(state.nitrite_mg_n_per_l(), 0.7);
    assert_close(state.nitrate_mg_n_per_l(), 3.0);
    assert_close(state.don_mg_n_per_l(), 1.2);
    assert_close(state.doc_mg_c_per_l(), 5.0);
    assert_close(state.dic_mg_c_per_l(), 12.0);
    assert_close(state.do_mg_per_l(), 8.0);
    assert_close(state.phosphate_mg_p_per_l(), 0.9);
    assert_close(state.alkalinity_meq_per_l(), 2.5);
    assert_close(state.calcium_mg_per_l(), 20.0);
    assert_close(state.magnesium_mg_per_l(), 5.0);
    assert_close(state.gh_d(), expected_gh_d(20.0, 5.0));
    assert_close(state.kh_d(), expected_kh_d(2.5));
    assert_close(state.tds_mg_per_l(), 128.0);
    assert_close(state.conductivity_us_cm(), 128.0 / 0.65);
}

#[test]
fn helpers_gracefully_handle_zero_volume_from_full_displacement() {
    let mut state = helper_state();
    state.geometry.fill_height_cm = 2.0;
    state.water.ammonia_total_mg_n_total = 10.0;
    state.water.nitrite_mg_n_total = 10.0;
    state.water.nitrate_mg_n_total = 10.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 10.0;
    state.water.dissolved_organic_carbon_mg_c_total = 10.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 10.0;
    state.water.dissolved_oxygen_mg_total = 10.0;
    state.water.phosphate_mg_p_total = 10.0;
    state.water.alkalinity_meq_total = 10.0;
    state.water.calcium_mg_total = 10.0;
    state.water.magnesium_mg_total = 10.0;
    state.water.sodium_mg_total = 10.0;
    state.water.potassium_mg_total = 10.0;
    state.water.bicarbonate_mg_total = 10.0;
    state.water.chloride_mg_total = 10.0;
    state.water.sulfate_mg_total = 10.0;

    assert_close(state.water_volume_l(), 0.0);
    assert_close(state.tan_mg_n_per_l(), 0.0);
    assert_close(state.nitrite_mg_n_per_l(), 0.0);
    assert_close(state.nitrate_mg_n_per_l(), 0.0);
    assert_close(state.don_mg_n_per_l(), 0.0);
    assert_close(state.doc_mg_c_per_l(), 0.0);
    assert_close(state.dic_mg_c_per_l(), 0.0);
    assert_close(state.do_mg_per_l(), 0.0);
    assert_close(state.phosphate_mg_p_per_l(), 0.0);
    assert_close(state.alkalinity_meq_per_l(), 0.0);
    assert_close(state.calcium_mg_per_l(), 0.0);
    assert_close(state.magnesium_mg_per_l(), 0.0);
    assert_close(state.gh_d(), 0.0);
    assert_close(state.kh_d(), 0.0);
    assert_close(state.tds_mg_per_l(), 0.0);
    assert_close(state.conductivity_us_cm(), 0.0);
}

#[test]
fn helpers_gracefully_handle_empty_tank_geometry() {
    let mut state = helper_state();
    state.geometry.fill_height_cm = 0.0;
    state.substrate_layers.clear();
    state.water.ammonia_total_mg_n_total = 10.0;

    assert_close(state.water_volume_l(), 0.0);
    assert_close(state.tan_mg_n_per_l(), 0.0);
    assert_close(state.do_mg_per_l(), 0.0);
}

#[test]
fn helpers_remain_stable_for_very_large_tanks() {
    let mut state = helper_state();
    state.geometry = TankGeometry {
        length_cm: 10_000.0,
        width_cm: 10_000.0,
        height_cm: 200.0,
        fill_height_cm: 100.0,
        glass_thickness_mm: 12.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    state.substrate_layers = vec![SubstrateLayerState {
        depth_cm: 10.0,
        colonizable_area_cm2: state.geometry.footprint_area_cm2(),
        ..SubstrateLayerState::default()
    }];
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.ammonia_total_mg_n_total = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * state.water_volume_l();

    assert_close(state.substrate_volume_l(), 1_000_000.0);
    assert_close(state.water_volume_l(), 9_000_000.0);
    assert_close(state.tan_mg_n_per_l(), 1.0);
    assert_close(state.do_mg_per_l(), 8.0);
}

#[test]
fn geometry_water_constructors_use_net_water_volume() {
    let geometry = TankGeometry {
        length_cm: 50.0,
        width_cm: 20.0,
        height_cm: 15.0,
        fill_height_cm: 12.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    let substrate_depth_cm = 2.0;
    let net_volume_l = geometry.water_volume_l_with_substrate_depth(substrate_depth_cm);
    let default_water = WaterState::default_for_geometry(&geometry, substrate_depth_cm);
    assert_close(net_volume_l, 10.0);
    assert_close(default_water.do_mg_per_l(net_volume_l), 8.0);
    assert_close(default_water.dic_mg_c_per_l(net_volume_l), 20.0);
    assert_close(default_water.alkalinity_meq_per_l(net_volume_l), 1.5);

    let profile = SourceWaterProfile {
        temperature_c: 25.0,
        ammonia_mg_n_per_l: 1.5,
        nitrite_mg_n_per_l: 0.2,
        nitrate_mg_n_per_l: 12.0,
        phosphate_mg_p_per_l: 0.8,
        dic_mg_c_per_l: 18.0,
        doc_mg_c_per_l: 3.0,
        don_mg_n_per_l: 0.7,
        alkalinity_meq_per_l: 2.2,
        calcium_mg_per_l: 28.0,
        magnesium_mg_per_l: 7.0,
        sodium_mg_per_l: 11.0,
        potassium_mg_per_l: 4.0,
        bicarbonate_mg_per_l: 90.0,
        chloride_mg_per_l: 16.0,
        sulfate_mg_per_l: 9.0,
    };
    let source_water = WaterState::from_source_profile(&profile, &geometry, substrate_depth_cm);

    assert_close(
        source_water.tan_mg_n_per_l(net_volume_l),
        profile.ammonia_mg_n_per_l,
    );
    assert_close(
        source_water.nitrate_mg_n_per_l(net_volume_l),
        profile.nitrate_mg_n_per_l,
    );
    assert_close(
        source_water.don_mg_n_per_l(net_volume_l),
        profile.don_mg_n_per_l,
    );
    assert_close(
        source_water.alkalinity_meq_per_l(net_volume_l),
        profile.alkalinity_meq_per_l,
    );
    assert_close(
        source_water.kh_d(net_volume_l),
        expected_kh_d(profile.alkalinity_meq_per_l),
    );
}

#[test]
fn default_water_state_matches_default_tank_net_volume() {
    let state = TankState::new(SimSeed(21));
    let water = WaterState::default();
    let volume_l = state.water_volume_l();

    assert_close(water.do_mg_per_l(volume_l), 8.0);
    assert_close(water.dic_mg_c_per_l(volume_l), 20.0);
    assert_close(water.alkalinity_meq_per_l(volume_l), 1.5);
    assert_close(water.gh_d(volume_l), state.gh_d());
    assert_close(water.kh_d(volume_l), state.kh_d());
}

#[test]
fn seeded_example_reseeds_default_chemistry_for_its_current_volume() {
    let state = TankState::seeded_example(SimSeed(7));
    let volume_l = state.water_volume_l();
    let expected = WaterState::default_for_volume_l(volume_l);

    assert_close(state.do_mg_per_l(), expected.do_mg_per_l(volume_l));
    assert_close(state.dic_mg_c_per_l(), expected.dic_mg_c_per_l(volume_l));
    assert_close(
        state.alkalinity_meq_per_l(),
        expected.alkalinity_meq_per_l(volume_l),
    );
    assert_close(state.gh_d(), expected.gh_d(volume_l));
    assert_close(state.kh_d(), expected.kh_d(volume_l));
    assert_close(state.stability_tracker.prev_do_mg_l, state.do_mg_per_l());
    assert_close(state.stability_tracker.prev_gh_d, state.gh_d());
}

#[test]
fn concentration_view_matches_tank_helpers_without_recomputing_volume() {
    let mut state = helper_state();
    state.water.ammonia_total_mg_n_total = 15.0;
    state.water.nitrate_mg_n_total = 30.0;
    state.water.dissolved_oxygen_mg_total = 80.0;
    state.water.alkalinity_meq_total = 25.0;

    let chemistry = state.concentrations();

    assert_close(chemistry.volume_l(), state.water_volume_l());
    assert_close(chemistry.tan_mg_n_per_l(), state.tan_mg_n_per_l());
    assert_close(chemistry.nitrate_mg_n_per_l(), state.nitrate_mg_n_per_l());
    assert_close(chemistry.do_mg_per_l(), state.do_mg_per_l());
    assert_close(chemistry.kh_d(), state.kh_d());
}

proptest! {
    #[test]
    fn helper_outputs_never_go_negative(
        length_cm in 1.0_f64..500.0,
        width_cm in 1.0_f64..500.0,
        fill_height_cm in 0.0_f64..200.0,
        substrate_depth_cm in 0.0_f64..250.0,
        ammonia_total in 0.0_f64..1.0e6,
        nitrite_total in 0.0_f64..1.0e6,
        nitrate_total in 0.0_f64..1.0e6,
        don_total in 0.0_f64..1.0e6,
        doc_total in 0.0_f64..1.0e6,
        dic_total in 0.0_f64..1.0e6,
        do_total in 0.0_f64..1.0e6,
        phosphate_total in 0.0_f64..1.0e6,
        alkalinity_total in 0.0_f64..1.0e6,
        calcium_total in 0.0_f64..1.0e6,
        magnesium_total in 0.0_f64..1.0e6,
        sodium_total in 0.0_f64..1.0e6,
        potassium_total in 0.0_f64..1.0e6,
        bicarbonate_total in 0.0_f64..1.0e6,
        chloride_total in 0.0_f64..1.0e6,
        sulfate_total in 0.0_f64..1.0e6,
    ) {
        let mut state = TankState::new(SimSeed(99));
        state.geometry = TankGeometry {
            length_cm,
            width_cm,
            height_cm: fill_height_cm + 10.0,
            fill_height_cm,
            glass_thickness_mm: 5.0,
            open_top: true,
            lid_exchange_factor: 0.25,
        };
        state.substrate_layers = vec![SubstrateLayerState {
            depth_cm: substrate_depth_cm,
            colonizable_area_cm2: state.geometry.footprint_area_cm2(),
            ..SubstrateLayerState::default()
        }];
        state.water.ammonia_total_mg_n_total = ammonia_total;
        state.water.nitrite_mg_n_total = nitrite_total;
        state.water.nitrate_mg_n_total = nitrate_total;
        state.water.dissolved_organic_nitrogen_mg_n_total = don_total;
        state.water.dissolved_organic_carbon_mg_c_total = doc_total;
        state.water.dissolved_inorganic_carbon_mg_c_total = dic_total;
        state.water.dissolved_oxygen_mg_total = do_total;
        state.water.phosphate_mg_p_total = phosphate_total;
        state.water.alkalinity_meq_total = alkalinity_total;
        state.water.calcium_mg_total = calcium_total;
        state.water.magnesium_mg_total = magnesium_total;
        state.water.sodium_mg_total = sodium_total;
        state.water.potassium_mg_total = potassium_total;
        state.water.bicarbonate_mg_total = bicarbonate_total;
        state.water.chloride_mg_total = chloride_total;
        state.water.sulfate_mg_total = sulfate_total;

        prop_assert!(state.substrate_volume_l() >= 0.0);
        prop_assert!(state.water_volume_l() >= 0.0);
        prop_assert!(state.tan_mg_n_per_l() >= 0.0);
        prop_assert!(state.nitrite_mg_n_per_l() >= 0.0);
        prop_assert!(state.nitrate_mg_n_per_l() >= 0.0);
        prop_assert!(state.don_mg_n_per_l() >= 0.0);
        prop_assert!(state.doc_mg_c_per_l() >= 0.0);
        prop_assert!(state.dic_mg_c_per_l() >= 0.0);
        prop_assert!(state.do_mg_per_l() >= 0.0);
        prop_assert!(state.phosphate_mg_p_per_l() >= 0.0);
        prop_assert!(state.alkalinity_meq_per_l() >= 0.0);
        prop_assert!(state.calcium_mg_per_l() >= 0.0);
        prop_assert!(state.magnesium_mg_per_l() >= 0.0);
        prop_assert!(state.gh_d() >= 0.0);
        prop_assert!(state.kh_d() >= 0.0);
        prop_assert!(state.tds_mg_per_l() >= 0.0);
        prop_assert!(state.conductivity_us_cm() >= 0.0);
    }
}
