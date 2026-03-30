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
    if volume_l <= f64::EPSILON {
        return;
    }

    let do_mg_per_cm3 = (state.water.dissolved_oxygen_mg_total / volume_l) / 1000.0;
    let temperature_c = state.water.temperature_c;
    let r_total = estimate_substrate_o2_demand_rate(state);

    for layer in &mut state.substrate_layers {
        layer.o2_penetration_depth_cm = compute_o2_penetration_depth_cm(
            layer.resolved_porosity(),
            layer.depth_cm,
            do_mg_per_cm3,
            temperature_c,
            r_total,
        );
    }
}

/// Bouldin penetration depth for a single layer.
fn compute_o2_penetration_depth_cm(
    porosity: f64,
    layer_depth_cm: f64,
    do_mg_per_cm3: f64,
    temperature_c: f64,
    r_total_mg_per_cm3_per_s: f64,
) -> f64 {
    let d_free = d_o2_free_cm2_per_s(temperature_c);
    let d_eff = d_free * porosity * porosity;

    if r_total_mg_per_cm3_per_s < MIN_R_TOTAL_MG_PER_CM3_PER_S || do_mg_per_cm3 <= 0.0 {
        return layer_depth_cm.max(0.0);
    }

    let penetration = (2.0 * d_eff * do_mg_per_cm3.max(0.0) / r_total_mg_per_cm3_per_s).sqrt();

    penetration.clamp(0.0, layer_depth_cm.max(0.0))
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
    use crate::{rng::SimSeed, types::SubstrateKind, TankState};

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

        for layer in &state.substrate_layers {
            let depth = layer.depth_cm;
            assert!(
                (layer.o2_penetration_depth_cm - depth).abs() < f64::EPSILON,
                "expected full-depth penetration ({depth} cm) with zero demand, got {:.4} cm",
                layer.o2_penetration_depth_cm
            );
        }
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
        for layer in &state.substrate_layers {
            let total = layer.total_pore_volume_cm3(footprint);
            let oxic = layer.oxic_pore_volume_cm3(footprint);
            let suboxic = layer.suboxic_pore_volume_cm3(footprint);
            assert!(
                (oxic + suboxic - total).abs() < 1e-9,
                "zone pore volumes ({oxic:.6} + {suboxic:.6} = {:.6}) != total ({total:.6})",
                oxic + suboxic
            );
        }
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
        use crate::types::habitat::find_habitat;
        use crate::types::{HabitatKind, SubstrateLayerState};

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
        let layer = &state.substrate_layers[0];

        // Verify zone depths.
        assert!(
            (layer.oxic_depth_cm() - 2.0).abs() < 1e-9,
            "oxic depth should be 2.0 cm"
        );
        assert!(
            (layer.suboxic_depth_cm() - 3.0).abs() < 1e-9,
            "suboxic depth should be 3.0 cm"
        );

        // SubstrateSurface habitat area = footprint + oxic interstitial.
        let surface = find_habitat(&state.habitat_registry, HabitatKind::SubstrateSurface)
            .expect("SubstrateSurface should exist");
        let expected_surface_area = footprint + layer.oxic_colonizable_area_cm2(footprint);
        assert!(
            (surface.colonizable_area_cm2 - expected_surface_area).abs() < 1e-6,
            "SubstrateSurface area ({}) should equal footprint + oxic interstitial ({})",
            surface.colonizable_area_cm2,
            expected_surface_area,
        );

        // SubstrateDeep habitat area = suboxic interstitial.
        let deep = find_habitat(&state.habitat_registry, HabitatKind::SubstrateDeep)
            .expect("SubstrateDeep should exist");
        let expected_deep_area = layer.suboxic_colonizable_area_cm2(footprint);
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
}
