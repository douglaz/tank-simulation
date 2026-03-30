use crate::types::{HabitatKind, PlantGuild, TankState};

/// Free-water O₂ diffusion coefficient at 20 °C (cm²/s).
///
/// Temperature dependence is approximated linearly: +1.5 %/°C above 20 °C.
/// Source: Broecker & Peng, *Tracers in the Sea*, 1982.
const D_O2_FREE_20C_CM2_PER_S: f64 = 2.0e-5;

/// Minimum volumetric O₂ consumption rate (mg O₂ cm⁻³ s⁻¹) to avoid
/// division-by-zero in the Bouldin penetration model. When biological
/// demand is below this floor the entire substrate is treated as oxic.
const MIN_R_TOTAL_MG_PER_CM3_PER_S: f64 = 1e-12;

/// Update `o2_penetration_depth_cm` on every substrate layer from the
/// Bouldin (1968) one-dimensional steady-state diffusion model:
///
///   z = sqrt(2 × D_eff × \[O₂\]_surface / R_total)
///
/// where
///   D_eff = D_free × porosity²  (tortuosity correction)
///   \[O₂\]_surface = water-column dissolved oxygen (mg cm⁻³)
///   R_total = biological O₂ demand per cm³ of bulk substrate (mg cm⁻³ s⁻¹)
pub fn step_substrate_zones(state: &mut TankState) {
    let volume_l = state.water_volume_l();
    let total_depth_cm = state.substrate_depth_cm();
    if volume_l <= f64::EPSILON || total_depth_cm <= f64::EPSILON {
        return;
    }

    let do_mg_per_cm3 = (state.water.dissolved_oxygen_mg_total / volume_l) / 1000.0;
    let temperature_c = state.water.temperature_c;
    let r_total = estimate_substrate_o2_demand_rate(state);
    let effective_porosity = effective_substrate_porosity(state);
    let o2_penetration_depth_cm = compute_o2_penetration_depth_cm(
        effective_porosity,
        total_depth_cm,
        do_mg_per_cm3,
        temperature_c,
        r_total,
    );

    for layer in &mut state.substrate_layers {
        layer.o2_penetration_depth_cm = o2_penetration_depth_cm;
    }
}

/// Bouldin penetration depth for the full stacked substrate bed.
fn compute_o2_penetration_depth_cm(
    porosity: f64,
    substrate_depth_cm: f64,
    do_mg_per_cm3: f64,
    temperature_c: f64,
    r_total_mg_per_cm3_per_s: f64,
) -> f64 {
    let substrate_depth_cm = substrate_depth_cm.max(0.0);
    if substrate_depth_cm <= f64::EPSILON {
        return 0.0;
    }

    let d_free = d_o2_free_cm2_per_s(temperature_c);
    let d_eff = d_free * porosity * porosity;

    if do_mg_per_cm3 <= 0.0 {
        return 0.0;
    }
    if r_total_mg_per_cm3_per_s < MIN_R_TOTAL_MG_PER_CM3_PER_S {
        return substrate_depth_cm;
    }

    let penetration = (2.0 * d_eff * do_mg_per_cm3.max(0.0) / r_total_mg_per_cm3_per_s).sqrt();

    penetration.clamp(0.0, substrate_depth_cm)
}

/// Temperature-adjusted free-water O₂ diffusion coefficient (cm²/s).
fn d_o2_free_cm2_per_s(temperature_c: f64) -> f64 {
    let t = temperature_c.clamp(0.0, 40.0);
    D_O2_FREE_20C_CM2_PER_S * (1.0 + 0.015 * (t - 20.0))
}

/// Estimate the volumetric biological O₂ demand rate in the substrate
/// (mg O₂ cm⁻³ s⁻¹) from decomposer activity and root respiration.
///
/// The demand is derived from:
///   1. Decomposer biomass × BOD rate × substrate habitat fraction
///   2. Root respiration from rooted plant biomass
///
/// The total demand is distributed over the bulk substrate volume.
fn estimate_substrate_o2_demand_rate(state: &TankState) -> f64 {
    let footprint_cm2 = state.geometry.footprint_area_cm2();
    let total_depth_cm: f64 = state
        .substrate_layers
        .iter()
        .map(|l| l.depth_cm.max(0.0))
        .sum();
    let substrate_bulk_volume_cm3 = footprint_cm2 * total_depth_cm;
    if substrate_bulk_volume_cm3 <= f64::EPSILON {
        return 0.0;
    }

    let bod_rate_per_s = state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour
        / 3600.0;

    // Fraction of decomposer biomass residing in substrate habitats,
    // estimated from colonizable area distribution.
    let substrate_fraction = substrate_habitat_area_fraction(state);

    let decomposer_demand =
        state.microbe.decomposer_biomass_g * bod_rate_per_s * substrate_fraction;

    // Root respiration: rooted plant biomass contributes O₂ demand in
    // the substrate via root metabolism (roughly 10% of above-ground
    // respiration rate as a first-pass proxy).
    let rooted_biomass_g: f64 = state
        .plant_guilds
        .iter()
        .filter(|p| matches!(p.guild, PlantGuild::RootFeedingRosette))
        .map(|p| p.biomass_g)
        .sum();
    let root_demand = rooted_biomass_g * bod_rate_per_s * 0.1;

    (decomposer_demand + root_demand) / substrate_bulk_volume_cm3
}

fn effective_substrate_porosity(state: &TankState) -> f64 {
    let total_depth_cm = state.substrate_depth_cm();
    if total_depth_cm <= f64::EPSILON {
        return 0.0;
    }

    state
        .substrate_layers
        .iter()
        .map(|layer| layer.depth_cm.max(0.0) * layer.resolved_porosity())
        .sum::<f64>()
        / total_depth_cm
}

/// Fraction of total colonizable area that belongs to substrate habitats.
fn substrate_habitat_area_fraction(state: &TankState) -> f64 {
    let total_area: f64 = state
        .habitat_registry
        .iter()
        .map(|h| h.colonizable_area_cm2)
        .sum();
    if total_area <= f64::EPSILON {
        return 0.3;
    }
    let substrate_area: f64 = state
        .habitat_registry
        .iter()
        .filter(|h| {
            matches!(
                h.kind,
                HabitatKind::SubstrateSurface | HabitatKind::SubstrateDeep
            )
        })
        .map(|h| h.colonizable_area_cm2)
        .sum();
    (substrate_area / total_area).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use crate::{
        find_habitat, rng::SimSeed, HabitatKind, SubstrateKind, SubstrateLayerState, TankState,
    };

    use super::*;

    fn make_state() -> TankState {
        TankState::new(SimSeed(70_001))
    }

    #[test]
    fn high_do_low_demand_yields_deep_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        // Use porous substrate with 5 cm depth so penetration can exceed 3 cm.
        state.substrate_layers = vec![crate::types::SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            depth_cm: 5.0,
            porosity: SubstrateKind::CoarsePorous.default_porosity(),
            o2_penetration_depth_cm: 5.0,
            ..crate::types::SubstrateLayerState::default()
        }];
        state.refresh_habitat_registry();
        // High DO: 8 mg/L
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        // Very low decomposer biomass → minimal demand
        state.microbe.decomposer_biomass_g = 0.001;
        state.plant_guilds.clear();
        step_substrate_zones(&mut state);

        let pen = state.substrate_layers[0].o2_penetration_depth_cm;
        assert!(
            pen > 3.0,
            "expected deep penetration (> 3 cm) in porous substrate with high DO and low demand, got {pen:.4} cm",
        );
        Ok(())
    }

    #[test]
    fn low_do_high_demand_yields_shallow_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        let volume_l = state.water_volume_l();
        // Low DO: 2 mg/L
        state.water.dissolved_oxygen_mg_total = 2.0 * volume_l;
        // High decomposer biomass → strong substrate O2 demand
        state.microbe.decomposer_biomass_g = 10.0;
        step_substrate_zones(&mut state);

        for layer in &state.substrate_layers {
            assert!(
                layer.o2_penetration_depth_cm < 1.0,
                "expected shallow penetration (< 1 cm) with low DO and high demand, got {:.4} cm",
                layer.o2_penetration_depth_cm
            );
        }
        Ok(())
    }

    #[test]
    fn zero_demand_yields_full_depth_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        // Zero biological activity
        state.microbe.decomposer_biomass_g = 0.0;
        state.plant_guilds.clear();
        step_substrate_zones(&mut state);

        let total_depth = state.substrate_depth_cm();
        assert!(
            (state.substrate_o2_penetration_depth_cm() - total_depth).abs() < f64::EPSILON,
            "expected full-depth penetration ({total_depth} cm) with zero demand, got {:.4} cm",
            state.substrate_o2_penetration_depth_cm()
        );
        Ok(())
    }

    #[test]
    fn zone_volumes_sum_to_total_pore_volume() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 5.0 * volume_l;
        state.microbe.decomposer_biomass_g = 0.5;
        step_substrate_zones(&mut state);

        let footprint = state.geometry.footprint_area_cm2();
        let total_pore_volume: f64 = state
            .substrate_layers
            .iter()
            .map(|layer| layer.total_pore_volume_cm3(footprint))
            .sum();
        let zoned_pore_volume =
            state.substrate_oxic_pore_volume_cm3() + state.substrate_suboxic_pore_volume_cm3();
        assert!(
            (zoned_pore_volume - total_pore_volume).abs() < 1e-9,
            "zone pore volumes ({zoned_pore_volume:.6}) should equal total ({total_pore_volume:.6})",
        );
        Ok(())
    }

    #[test]
    fn changing_porosity_changes_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 6.0 * volume_l;
        state.microbe.decomposer_biomass_g = 0.3;

        // Low porosity
        for layer in &mut state.substrate_layers {
            layer.porosity = 0.2;
        }
        step_substrate_zones(&mut state);
        let low_porosity_penetration = state.substrate_layers[0].o2_penetration_depth_cm;

        // High porosity
        for layer in &mut state.substrate_layers {
            layer.porosity = 0.6;
        }
        step_substrate_zones(&mut state);
        let high_porosity_penetration = state.substrate_layers[0].o2_penetration_depth_cm;

        assert!(
            high_porosity_penetration > low_porosity_penetration,
            "higher porosity should yield deeper penetration: high={high_porosity_penetration:.4}, low={low_porosity_penetration:.4}"
        );
        Ok(())
    }

    #[test]
    fn penetration_depth_decreases_with_bioload() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        // Use deeper substrate so penetration doesn't clamp at layer depth.
        state.substrate_layers = vec![crate::types::SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            depth_cm: 8.0,
            porosity: SubstrateKind::CoarsePorous.default_porosity(),
            o2_penetration_depth_cm: 8.0,
            ..crate::types::SubstrateLayerState::default()
        }];
        state.refresh_habitat_registry();
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 5.0 * volume_l;

        state.microbe.decomposer_biomass_g = 0.5;
        step_substrate_zones(&mut state);
        let low_load = state.substrate_layers[0].o2_penetration_depth_cm;

        state.microbe.decomposer_biomass_g = 5.0;
        step_substrate_zones(&mut state);
        let high_load = state.substrate_layers[0].o2_penetration_depth_cm;

        assert!(
            low_load > high_load,
            "higher bioload should reduce penetration: low_load={low_load:.4}, high_load={high_load:.4}"
        );
        Ok(())
    }

    #[test]
    fn initial_zone_proportions_by_substrate_kind() -> Result<(), Box<dyn std::error::Error>> {
        // Fine sand has lower porosity → shallower oxic zone under equal conditions.
        // Coarse porous has higher porosity → deeper oxic zone.
        let do_mg_per_cm3 = 0.007; // 7 mg/L
        let r_total = 5e-8; // moderate demand
        let temp = 25.0;
        let depth = 5.0;

        let pen_sand = compute_o2_penetration_depth_cm(
            SubstrateKind::InertSand.default_porosity(),
            depth,
            do_mg_per_cm3,
            temp,
            r_total,
        );
        let pen_gravel = compute_o2_penetration_depth_cm(
            SubstrateKind::InertGravel.default_porosity(),
            depth,
            do_mg_per_cm3,
            temp,
            r_total,
        );
        let pen_coarse = compute_o2_penetration_depth_cm(
            SubstrateKind::CoarsePorous.default_porosity(),
            depth,
            do_mg_per_cm3,
            temp,
            r_total,
        );

        assert!(
            pen_sand < pen_gravel,
            "sand ({pen_sand:.4}) should have shallower penetration than gravel ({pen_gravel:.4})"
        );
        assert!(
            pen_gravel < pen_coarse,
            "gravel ({pen_gravel:.4}) should have shallower penetration than coarse ({pen_coarse:.4})"
        );
        Ok(())
    }

    #[test]
    fn zone_habitat_integration_oxic_maps_to_surface_suboxic_maps_to_deep(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        // Set up a substrate with partial O₂ penetration.
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            depth_cm: 5.0,
            porosity: SubstrateKind::CoarsePorous.default_porosity(),
            o2_penetration_depth_cm: 2.0, // 2 cm oxic, 3 cm suboxic
            ..SubstrateLayerState::default()
        }];
        state.refresh_habitat_registry();

        let footprint = state.geometry.footprint_area_cm2();
        let oxic = state.substrate_oxic_zone_geometry();
        let suboxic = state.substrate_suboxic_zone_geometry();

        // Verify zone depths.
        assert!(
            (oxic.depth_cm - 2.0).abs() < 1e-9,
            "oxic depth should be 2.0 cm"
        );
        assert!(
            (suboxic.depth_cm - 3.0).abs() < 1e-9,
            "suboxic depth should be 3.0 cm"
        );

        // SubstrateSurface habitat area = footprint + oxic interstitial.
        let surface = find_habitat(&state.habitat_registry, HabitatKind::SubstrateSurface)
            .expect("SubstrateSurface should exist");
        let expected_surface_area = footprint + oxic.colonizable_area_cm2;
        assert!(
            (surface.colonizable_area_cm2 - expected_surface_area).abs() < 1e-6,
            "SubstrateSurface area ({}) should equal footprint + oxic interstitial ({})",
            surface.colonizable_area_cm2,
            expected_surface_area,
        );

        // SubstrateDeep habitat area = suboxic interstitial.
        let deep = find_habitat(&state.habitat_registry, HabitatKind::SubstrateDeep)
            .expect("SubstrateDeep should exist");
        let expected_deep_area = suboxic.colonizable_area_cm2;
        assert!(
            (deep.colonizable_area_cm2 - expected_deep_area).abs() < 1e-6,
            "SubstrateDeep area ({}) should equal suboxic interstitial ({})",
            deep.colonizable_area_cm2,
            expected_deep_area,
        );

        // Deep > 0 since we have 3 cm of suboxic zone.
        assert!(
            deep.colonizable_area_cm2 > 0.0,
            "SubstrateDeep area should be positive with partial penetration"
        );

        Ok(())
    }

    #[test]
    fn anoxic_water_yields_zero_penetration_and_positive_suboxic_volume(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.water.dissolved_oxygen_mg_total = 0.0;
        state.microbe.decomposer_biomass_g = 2.0;

        step_substrate_zones(&mut state);
        state.refresh_habitat_registry();

        assert!(
            state.substrate_o2_penetration_depth_cm().abs() < 1e-12,
            "anoxic water should produce zero penetration, got {:.6} cm",
            state.substrate_o2_penetration_depth_cm()
        );
        assert!(
            state.substrate_oxic_zone_geometry().volume_cm3.abs() < 1e-12,
            "anoxic water should leave no oxic substrate volume"
        );
        assert!(
            state.substrate_suboxic_zone_geometry().volume_cm3 > 0.0,
            "anoxic water should leave positive suboxic substrate volume"
        );

        Ok(())
    }

    #[test]
    fn stacked_layers_use_single_shared_penetration_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.substrate_layers = vec![
            SubstrateLayerState {
                kind: SubstrateKind::InertSand,
                depth_cm: 1.5,
                o2_penetration_depth_cm: 1.0,
                ..SubstrateLayerState::default()
            },
            SubstrateLayerState {
                kind: SubstrateKind::CoarsePorous,
                depth_cm: 3.0,
                o2_penetration_depth_cm: 1.0,
                ..SubstrateLayerState::default()
            },
        ];
        state.refresh_habitat_registry();

        let boundary = state.substrate_o2_penetration_depth_cm();
        let top = &state.substrate_layers[0];
        let bottom = &state.substrate_layers[1];

        assert!((boundary - 1.0).abs() < 1e-12);
        assert!((top.oxic_depth_cm(0.0, boundary) - 1.0).abs() < 1e-12);
        assert!((top.suboxic_depth_cm(0.0, boundary) - 0.5).abs() < 1e-12);
        assert!((bottom.oxic_depth_cm(1.5, boundary) - 0.0).abs() < 1e-12);
        assert!((bottom.suboxic_depth_cm(1.5, boundary) - 3.0).abs() < 1e-12);
        assert!(
            (state.substrate_oxic_zone_geometry().depth_cm - 1.0).abs() < 1e-12,
            "only the uppermost 1 cm should be oxic"
        );
        assert!(
            (state.substrate_suboxic_zone_geometry().depth_cm - 3.5).abs() < 1e-12,
            "the remainder of the stacked bed should be suboxic"
        );

        Ok(())
    }

    #[test]
    fn shallow_substrate_has_no_deep_zone() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.substrate_layers = vec![SubstrateLayerState {
            depth_cm: 0.1,
            kind: SubstrateKind::CoarsePorous,
            porosity: SubstrateKind::CoarsePorous.default_porosity(),
            ..SubstrateLayerState::default()
        }];
        state.refresh_habitat_registry();
        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        state.microbe.decomposer_biomass_g = 0.0;
        state.plant_guilds.clear();

        step_substrate_zones(&mut state);
        state.refresh_habitat_registry();

        assert!(
            state.substrate_suboxic_zone_geometry().volume_cm3.abs() < 1e-12,
            "1 mm substrate should have no meaningful deep zone"
        );
        assert!(
            find_habitat(&state.habitat_registry, HabitatKind::SubstrateDeep)
                .expect("deep habitat")
                .colonizable_area_cm2
                .abs()
                < 1e-12,
            "shallow substrate should leave no deep habitat area"
        );

        Ok(())
    }

    #[test]
    fn zone_depths_sum_to_total_substrate_depth() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.substrate_layers = vec![
            SubstrateLayerState {
                depth_cm: 2.0,
                o2_penetration_depth_cm: 1.2,
                ..SubstrateLayerState::default()
            },
            SubstrateLayerState {
                depth_cm: 1.0,
                o2_penetration_depth_cm: 1.2,
                ..SubstrateLayerState::default()
            },
        ];

        let total_depth = state.substrate_depth_cm();
        let oxic_depth = state.substrate_oxic_zone_geometry().depth_cm;
        let suboxic_depth = state.substrate_suboxic_zone_geometry().depth_cm;
        assert!(
            (oxic_depth + suboxic_depth - total_depth).abs() < 1e-12,
            "oxic + suboxic depths should equal total depth: oxic={oxic_depth}, suboxic={suboxic_depth}, total={total_depth}"
        );

        Ok(())
    }

    #[test]
    fn zone_nutrient_availability_sums_to_total_store() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.substrate_layers = vec![SubstrateLayerState {
            depth_cm: 5.0,
            nutrient_store_mg_n_total: 50.0,
            nutrient_store_mg_p_total: 10.0,
            o2_penetration_depth_cm: 2.0,
            ..SubstrateLayerState::default()
        }];

        let oxic = state.substrate_oxic_nutrient_availability();
        let suboxic = state.substrate_suboxic_nutrient_availability();

        assert!((oxic.nitrogen_mg_total - 20.0).abs() < 1e-12);
        assert!((suboxic.nitrogen_mg_total - 30.0).abs() < 1e-12);
        assert!((oxic.phosphorus_mg_total - 4.0).abs() < 1e-12);
        assert!((suboxic.phosphorus_mg_total - 6.0).abs() < 1e-12);
        assert!((oxic.nitrogen_mg_total + suboxic.nitrogen_mg_total - 50.0).abs() < 1e-12);
        assert!((oxic.phosphorus_mg_total + suboxic.phosphorus_mg_total - 10.0).abs() < 1e-12);
        assert!(
            ((oxic.nitrogen_mg_per_m2 + suboxic.nitrogen_mg_per_m2)
                - state.substrate_n_mg_n_per_m2())
            .abs()
                < 1e-12
        );
        assert!(
            ((oxic.phosphorus_mg_per_m2 + suboxic.phosphorus_mg_per_m2)
                - state.substrate_p_mg_p_per_m2())
            .abs()
                < 1e-12
        );

        Ok(())
    }
}
