use crate::systems::microfauna::{
    microfauna_periphyton_accessibility, MICROFAUNA_DETRITUS_DAILY_FRACTION,
    MICROFAUNA_O2_PER_MG_C_RESPIRED,
};
use crate::types::{algae_carbon_mg, detritus_carbon_mg, HabitatKind, PlantGuild, TankState};

/// Free-water O₂ diffusion coefficient at 20 °C (cm²/s).
///
/// Temperature dependence is approximated linearly: +1.5 %/°C above 20 °C.
/// Source: Broecker & Peng, *Tracers in the Sea*, 1982.
const D_O2_FREE_20C_CM2_PER_S: f64 = 2.0e-5;
const SECONDS_PER_DAY: f64 = 86_400.0;

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
    let base_penetration = compute_o2_penetration_depth_cm(
        effective_porosity,
        total_depth_cm,
        do_mg_per_cm3,
        temperature_c,
        r_total,
    );

    let root_oxygenation_bonus = root_oxygenation_bonus_cm(state);
    let o2_penetration_depth_cm =
        (base_penetration + root_oxygenation_bonus).clamp(0.0, total_depth_cm);

    tracing::debug!(
        base_penetration_cm = base_penetration,
        root_oxygenation_bonus_cm = root_oxygenation_bonus,
        effective_penetration_cm = o2_penetration_depth_cm,
        "substrate O₂ penetration: base {base_penetration:.4} + ROL {root_oxygenation_bonus:.4} = {o2_penetration_depth_cm:.4} cm"
    );

    for layer in &mut state.substrate_layers {
        layer.o2_penetration_depth_cm = o2_penetration_depth_cm;
    }
}

/// Root-zone oxygenation bonus from radial oxygen loss (ROL).
///
/// Rooted plants (RootFeedingRosette) transport O₂ from photosynthesis
/// through their roots into the substrate, creating micro-oxic zones.
/// The bonus uses a sqrt model for diminishing returns:
///
///   bonus = sqrt(root_biomass_g) × rol_rate_cm_per_g
///
/// Only rooted plant guilds contribute; floating or epiphytic plants do not.
pub fn root_oxygenation_bonus_cm(state: &TankState) -> f64 {
    let rooted_biomass_g: f64 = state
        .plant_guilds
        .iter()
        .filter(|p| matches!(p.guild, PlantGuild::RootFeedingRosette))
        .map(|p| p.biomass_g.max(0.0))
        .sum();

    if rooted_biomass_g <= f64::EPSILON {
        return 0.0;
    }

    let rol_rate = state.process_params.rol_rate_cm_per_g;
    rooted_biomass_g.sqrt() * rol_rate
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
/// (mg O₂ cm⁻³ s⁻¹) from decomposer activity, root respiration, and
/// substrate-associated microfauna respiration.
///
/// The demand is derived from:
///   1. Decomposer biomass × BOD rate × substrate habitat fraction
///   2. Root respiration from rooted plant biomass
///   3. Microfauna respiration tied to substrate periphyton/detritus use
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

    let microfauna_demand = estimate_substrate_microfauna_o2_demand_mg_per_s(state);

    (decomposer_demand + root_demand + microfauna_demand) / substrate_bulk_volume_cm3
}

fn effective_substrate_porosity(state: &TankState) -> f64 {
    // The penetration model currently treats the top substrate layer as the
    // controlling diffusion bottleneck for the full stack. That is a
    // conservative simplification when deeper layers are more porous.
    state
        .substrate_layers
        .iter()
        .find(|layer| layer.depth_cm > f64::EPSILON)
        .map(|layer| layer.resolved_porosity())
        .unwrap_or(0.0)
}

fn estimate_substrate_microfauna_o2_demand_mg_per_s(state: &TankState) -> f64 {
    let population_index = state.microfauna.population_index.clamp(0.0, 1.0);
    if population_index <= f64::EPSILON {
        return 0.0;
    }

    let substrate_surface_access = (0.6
        + 0.4 * state.avg_substrate_index(|layer| layer.grazing_surface_index))
    .clamp(0.0, 1.0);
    let periphyton_consumption_fraction = state
        .process_params
        .microfauna_periphyton_consumption
        .max(0.0)
        * population_index;
    let requested_periphyton_g =
        state.algae.periphyton_biomass_g.max(0.0) * periphyton_consumption_fraction;

    let (accessible_substrate_periphyton_g, total_accessible_periphyton_g) = state
        .algae
        .periphyton_by_habitat
        .iter()
        .fold((0.0, 0.0), |(substrate, total), (kind, biomass_g)| {
            let accessibility =
                microfauna_periphyton_accessibility(substrate_surface_access, *kind);
            let accessible_biomass_g = biomass_g.max(0.0) * accessibility;
            let substrate = if *kind == HabitatKind::SubstrateSurface {
                substrate + accessible_biomass_g
            } else {
                substrate
            };
            (substrate, total + accessible_biomass_g)
        });

    let substrate_periphyton_consumed_g = if total_accessible_periphyton_g <= f64::EPSILON {
        0.0
    } else {
        requested_periphyton_g.min(total_accessible_periphyton_g)
            * accessible_substrate_periphyton_g
            / total_accessible_periphyton_g
    };

    let substrate_detritus_consumed_g = state.detritus.fine_detritus_g_total.max(0.0)
        * MICROFAUNA_DETRITUS_DAILY_FRACTION
        * population_index
        * substrate_habitat_area_fraction(state);

    let consumed_c_mg = algae_carbon_mg(
        substrate_periphyton_consumed_g,
        state.process_params.feed_n_to_c_ratio,
    ) + detritus_carbon_mg(
        substrate_detritus_consumed_g,
        state.process_params.feed_n_to_c_ratio,
    );
    if consumed_c_mg <= f64::EPSILON {
        return 0.0;
    }

    let assimilated_c_mg = consumed_c_mg
        * state
            .process_params
            .microfauna_assimilation_efficiency
            .clamp(0.0, 1.0);
    let respired_c_mg_per_day = assimilated_c_mg
        * state
            .process_params
            .microfauna_respiration_fraction_of_assimilated
            .clamp(0.0, 1.0);

    respired_c_mg_per_day * MICROFAUNA_O2_PER_MG_C_RESPIRED / SECONDS_PER_DAY
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
    use std::collections::BTreeMap;

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
    fn surface_layer_porosity_limits_heterogeneous_stack_penetration(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut low_porosity_cap = make_state();
        low_porosity_cap.substrate_layers = vec![
            SubstrateLayerState {
                kind: SubstrateKind::InertSand,
                depth_cm: 2.0,
                porosity: 0.25,
                ..SubstrateLayerState::default()
            },
            SubstrateLayerState {
                kind: SubstrateKind::CoarsePorous,
                depth_cm: 2.0,
                porosity: 0.55,
                ..SubstrateLayerState::default()
            },
        ];
        low_porosity_cap.refresh_habitat_registry();
        let volume_l = low_porosity_cap.water_volume_l();
        low_porosity_cap.water.dissolved_oxygen_mg_total = 4.0 * volume_l;
        low_porosity_cap.microbe.decomposer_biomass_g = 2.0;
        low_porosity_cap.plant_guilds.clear();
        low_porosity_cap.microfauna.population_index = 0.0;
        step_substrate_zones(&mut low_porosity_cap);

        let mut high_porosity_cap = low_porosity_cap.clone();
        high_porosity_cap.substrate_layers = vec![
            SubstrateLayerState {
                kind: SubstrateKind::CoarsePorous,
                depth_cm: 2.0,
                porosity: 0.55,
                ..SubstrateLayerState::default()
            },
            SubstrateLayerState {
                kind: SubstrateKind::InertSand,
                depth_cm: 2.0,
                porosity: 0.25,
                ..SubstrateLayerState::default()
            },
        ];
        high_porosity_cap.refresh_habitat_registry();
        step_substrate_zones(&mut high_porosity_cap);

        let low_cap_penetration = low_porosity_cap.substrate_o2_penetration_depth_cm();
        let high_cap_penetration = high_porosity_cap.substrate_o2_penetration_depth_cm();

        assert!(
            low_cap_penetration < high_cap_penetration,
            "a low-porosity surface cap should bottleneck diffusion: low_cap={low_cap_penetration:.4}, high_cap={high_cap_penetration:.4}"
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
    fn microfauna_respiration_shallows_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut without_microfauna = make_state();
        without_microfauna.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            depth_cm: 5.0,
            porosity: SubstrateKind::CoarsePorous.default_porosity(),
            ..SubstrateLayerState::default()
        }];
        without_microfauna.refresh_habitat_registry();
        let volume_l = without_microfauna.water_volume_l();
        without_microfauna.water.dissolved_oxygen_mg_total = 7.0 * volume_l;
        without_microfauna.microbe.decomposer_biomass_g = 0.0;
        without_microfauna.plant_guilds.clear();
        without_microfauna.detritus.fine_detritus_g_total = 3.0;
        without_microfauna.algae.periphyton_by_habitat = BTreeMap::from([
            (HabitatKind::SubstrateSurface, 10.0),
            (HabitatKind::GlassHardscape, 0.0),
            (HabitatKind::PlantSurfaces, 0.0),
            (HabitatKind::FilterMedia, 0.0),
        ]);
        without_microfauna.algae.sync_periphyton_total();
        without_microfauna.microfauna.population_index = 0.0;
        step_substrate_zones(&mut without_microfauna);

        let mut with_microfauna = without_microfauna.clone();
        with_microfauna.microfauna.population_index = 1.0;
        step_substrate_zones(&mut with_microfauna);

        let no_microfauna_penetration = without_microfauna.substrate_o2_penetration_depth_cm();
        let high_microfauna_penetration = with_microfauna.substrate_o2_penetration_depth_cm();

        assert!(
            high_microfauna_penetration < no_microfauna_penetration,
            "substrate microfauna respiration should shallow penetration: no_microfauna={no_microfauna_penetration:.4}, high_microfauna={high_microfauna_penetration:.4}"
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

    // -- Root-zone oxygenation (ROL) tests --

    fn make_rooted_plant(biomass_g: f64) -> crate::types::PlantGuildState {
        crate::types::PlantGuildState {
            guild: PlantGuild::RootFeedingRosette,
            biomass_g,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.5,
            water_column_uptake_bias: None,
            substrate_uptake_bias: None,
        }
    }

    fn make_floating_plant(biomass_g: f64) -> crate::types::PlantGuildState {
        crate::types::PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.95,
            water_column_uptake_bias: None,
            substrate_uptake_bias: None,
        }
    }

    #[test]
    fn rooted_plants_increase_o2_penetration() -> Result<(), Box<dyn std::error::Error>> {
        let mut unplanted = make_state();
        unplanted.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 8.0,
            porosity: SubstrateKind::ActivePlanted.default_porosity(),
            ..SubstrateLayerState::default()
        }];
        unplanted.refresh_habitat_registry();
        let volume_l = unplanted.water_volume_l();
        unplanted.water.dissolved_oxygen_mg_total = 7.0 * volume_l;
        unplanted.microbe.decomposer_biomass_g = 3.0;
        unplanted.plant_guilds.clear();

        let mut planted = unplanted.clone();
        planted.plant_guilds = vec![make_rooted_plant(10.0)];
        planted.refresh_habitat_registry();

        step_substrate_zones(&mut unplanted);
        step_substrate_zones(&mut planted);

        let pen_unplanted = unplanted.substrate_o2_penetration_depth_cm();
        let pen_planted = planted.substrate_o2_penetration_depth_cm();

        assert!(
            pen_planted > pen_unplanted,
            "rooted plants should deepen O₂ penetration: planted={pen_planted:.4}, unplanted={pen_unplanted:.4}"
        );
        Ok(())
    }

    #[test]
    fn root_oxygenation_proportional_to_biomass_with_diminishing_returns(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut base = make_state();
        base.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 8.0,
            porosity: SubstrateKind::ActivePlanted.default_porosity(),
            ..SubstrateLayerState::default()
        }];
        base.refresh_habitat_registry();
        let volume_l = base.water_volume_l();
        base.water.dissolved_oxygen_mg_total = 7.0 * volume_l;
        base.microbe.decomposer_biomass_g = 3.0;

        let mut low_biomass = base.clone();
        low_biomass.plant_guilds = vec![make_rooted_plant(2.0)];
        low_biomass.refresh_habitat_registry();

        let mut high_biomass = base.clone();
        high_biomass.plant_guilds = vec![make_rooted_plant(20.0)];
        high_biomass.refresh_habitat_registry();

        step_substrate_zones(&mut low_biomass);
        step_substrate_zones(&mut high_biomass);

        let pen_low = low_biomass.substrate_o2_penetration_depth_cm();
        let pen_high = high_biomass.substrate_o2_penetration_depth_cm();

        assert!(
            pen_high > pen_low,
            "more root biomass should deepen penetration: high={pen_high:.4}, low={pen_low:.4}"
        );

        // Diminishing returns: 10× more biomass should yield less than 10× bonus
        let bonus_low = root_oxygenation_bonus_cm(&low_biomass);
        let bonus_high = root_oxygenation_bonus_cm(&high_biomass);
        let ratio = bonus_high / bonus_low;
        let biomass_ratio = 20.0_f64 / 2.0;
        assert!(
            ratio < biomass_ratio,
            "bonus should show diminishing returns: bonus_ratio={ratio:.2}, biomass_ratio={biomass_ratio:.1}"
        );
        // With sqrt model, ratio should be sqrt(10) ≈ 3.16 for 10× biomass
        assert!(
            (ratio - biomass_ratio.sqrt()).abs() < 0.01,
            "bonus ratio should follow sqrt scaling: got {ratio:.4}, expected {:.4}",
            biomass_ratio.sqrt()
        );
        Ok(())
    }

    #[test]
    fn floating_plant_biomass_does_not_increase_penetration(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut base = make_state();
        base.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 8.0,
            porosity: SubstrateKind::ActivePlanted.default_porosity(),
            ..SubstrateLayerState::default()
        }];
        base.refresh_habitat_registry();
        let volume_l = base.water_volume_l();
        base.water.dissolved_oxygen_mg_total = 7.0 * volume_l;
        base.microbe.decomposer_biomass_g = 3.0;
        base.plant_guilds.clear();

        let mut with_floating = base.clone();
        with_floating.plant_guilds = vec![make_floating_plant(20.0)];
        with_floating.refresh_habitat_registry();

        step_substrate_zones(&mut base);
        step_substrate_zones(&mut with_floating);

        let bonus = root_oxygenation_bonus_cm(&with_floating);
        assert!(
            bonus.abs() < f64::EPSILON,
            "floating plants should produce zero ROL bonus: {bonus}"
        );

        // Penetration should be identical (floating plants add no ROL bonus,
        // though they may add slight O₂ demand differences from habitat changes).
        let pen_base = base.substrate_o2_penetration_depth_cm();
        let pen_floating = with_floating.substrate_o2_penetration_depth_cm();
        assert!(
            (pen_base - pen_floating).abs() < 0.1,
            "floating plants should not meaningfully change penetration: base={pen_base:.4}, floating={pen_floating:.4}"
        );
        Ok(())
    }

    #[test]
    fn zero_root_biomass_yields_zero_oxygenation_bonus() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.plant_guilds.clear();
        let bonus = root_oxygenation_bonus_cm(&state);
        assert!(
            bonus.abs() < f64::EPSILON,
            "zero root biomass should yield zero bonus: {bonus}"
        );

        // Also check with a rooted guild at zero biomass
        state.plant_guilds = vec![make_rooted_plant(0.0)];
        let bonus = root_oxygenation_bonus_cm(&state);
        assert!(
            bonus.abs() < f64::EPSILON,
            "zero-biomass rooted guild should yield zero bonus: {bonus}"
        );
        Ok(())
    }

    #[test]
    fn root_oxygenation_uses_named_parameter() -> Result<(), Box<dyn std::error::Error>> {
        let mut state = make_state();
        state.plant_guilds = vec![make_rooted_plant(10.0)];

        let default_rate = state.process_params.rol_rate_cm_per_g;
        let bonus_default = root_oxygenation_bonus_cm(&state);

        state.process_params.rol_rate_cm_per_g = default_rate * 2.0;
        let bonus_doubled = root_oxygenation_bonus_cm(&state);

        assert!(
            (bonus_doubled - bonus_default * 2.0).abs() < 1e-12,
            "bonus should scale linearly with rol_rate_cm_per_g: doubled={bonus_doubled:.4}, expected={:.4}",
            bonus_default * 2.0
        );
        Ok(())
    }
}
