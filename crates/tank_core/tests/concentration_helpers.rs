use proptest::prelude::*;
use tank_core::{SimSeed, SubstrateKind, SubstrateLayerState, TankGeometry, TankState, WaterState};

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

#[test]
fn canonical_helpers_use_net_water_volume() {
    let mut state = helper_state();
    assert_close(state.geometry.water_volume_l(), 12.0);
    assert_close(state.substrate_volume_l(), 2.0);
    assert_close(state.water_volume_l(), 10.0);

    state.water.ammonia_total_mg_n_total = 15.0;
    state.water.nitrite_mg_n_total = 7.0;
    state.water.nitrate_mg_n_total = 30.0;
    state.water.dissolved_organic_carbon_mg_c_total = 50.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 120.0;
    state.water.dissolved_oxygen_mg_total = 80.0;
    state.water.phosphate_mg_p_total = 9.0;
    state.water.alkalinity_meq_total = 25.0;

    assert_close(state.tan_mg_n_per_l(), 1.5);
    assert_close(state.nitrite_mg_n_per_l(), 0.7);
    assert_close(state.nitrate_mg_n_per_l(), 3.0);
    assert_close(state.doc_mg_c_per_l(), 5.0);
    assert_close(state.dic_mg_c_per_l(), 12.0);
    assert_close(state.do_mg_per_l(), 8.0);
    assert_close(state.phosphate_mg_p_per_l(), 0.9);
    assert_close(state.alkalinity_meq_per_l(), 2.5);
}

#[test]
fn helpers_gracefully_handle_zero_volume_from_full_displacement() {
    let mut state = helper_state();
    state.geometry.fill_height_cm = 2.0;
    state.water.ammonia_total_mg_n_total = 10.0;
    state.water.nitrite_mg_n_total = 10.0;
    state.water.nitrate_mg_n_total = 10.0;
    state.water.dissolved_organic_carbon_mg_c_total = 10.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 10.0;
    state.water.dissolved_oxygen_mg_total = 10.0;
    state.water.phosphate_mg_p_total = 10.0;
    state.water.alkalinity_meq_total = 10.0;

    assert_close(state.water_volume_l(), 0.0);
    assert_close(state.tan_mg_n_per_l(), 0.0);
    assert_close(state.nitrite_mg_n_per_l(), 0.0);
    assert_close(state.nitrate_mg_n_per_l(), 0.0);
    assert_close(state.doc_mg_c_per_l(), 0.0);
    assert_close(state.dic_mg_c_per_l(), 0.0);
    assert_close(state.do_mg_per_l(), 0.0);
    assert_close(state.phosphate_mg_p_per_l(), 0.0);
    assert_close(state.alkalinity_meq_per_l(), 0.0);
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
        doc_total in 0.0_f64..1.0e6,
        dic_total in 0.0_f64..1.0e6,
        do_total in 0.0_f64..1.0e6,
        phosphate_total in 0.0_f64..1.0e6,
        alkalinity_total in 0.0_f64..1.0e6,
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
        state.water.dissolved_organic_carbon_mg_c_total = doc_total;
        state.water.dissolved_inorganic_carbon_mg_c_total = dic_total;
        state.water.dissolved_oxygen_mg_total = do_total;
        state.water.phosphate_mg_p_total = phosphate_total;
        state.water.alkalinity_meq_total = alkalinity_total;

        prop_assert!(state.substrate_volume_l() >= 0.0);
        prop_assert!(state.water_volume_l() >= 0.0);
        prop_assert!(state.tan_mg_n_per_l() >= 0.0);
        prop_assert!(state.nitrite_mg_n_per_l() >= 0.0);
        prop_assert!(state.nitrate_mg_n_per_l() >= 0.0);
        prop_assert!(state.doc_mg_c_per_l() >= 0.0);
        prop_assert!(state.dic_mg_c_per_l() >= 0.0);
        prop_assert!(state.do_mg_per_l() >= 0.0);
        prop_assert!(state.phosphate_mg_p_per_l() >= 0.0);
        prop_assert!(state.alkalinity_meq_per_l() >= 0.0);
    }
}
