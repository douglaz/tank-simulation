use serde::{Deserialize, Serialize};

use super::{PlantGuild, TankState};
use crate::systems::light::{beer_lambert_at_depth, column_average_attenuation_factor};

/// Specific leaf area for FastStem guild (cm² of colonizable surface per gram).
const FAST_STEM_SPECIFIC_LEAF_AREA_CM2_PER_G: f64 = 40.0;

/// Specific leaf area for RootFeedingRosette guild (cm² per gram).
const ROOT_ROSETTE_SPECIFIC_LEAF_AREA_CM2_PER_G: f64 = 25.0;

/// Ecological zones where biofilm can colonize, decomposition can occur,
/// or redox conditions differ from the bulk water column.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HabitatKind {
    /// Biological filter media inside the filter housing.
    FilterMedia,
    /// Glass walls and any hardscape (rocks, driftwood) in the tank.
    GlassHardscape,
    /// Living plant leaf and stem surfaces available for epiphytic colonization.
    PlantSurfaces,
    /// The top surface of the substrate bed (tank footprint when substrate is present).
    SubstrateSurface,
    /// Internal grain surfaces within the substrate column (interstitial spaces).
    SubstrateDeep,
}

impl HabitatKind {
    /// All habitat kinds in canonical order.
    pub const ALL: [HabitatKind; 5] = [
        HabitatKind::FilterMedia,
        HabitatKind::GlassHardscape,
        HabitatKind::PlantSurfaces,
        HabitatKind::SubstrateSurface,
        HabitatKind::SubstrateDeep,
    ];
}

/// A single habitat entry describing colonizable area and normalized
/// environmental exposure modifiers.
///
/// All exposure modifiers are bounded in \[0, 1\] where 0 means no exposure
/// and 1 means maximum exposure. Downstream systems read these modifiers as
/// the canonical source for habitat-specific flow, oxygen, and light conditions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HabitatEntry {
    pub kind: HabitatKind,
    /// Total colonizable surface area (cm²).
    pub colonizable_area_cm2: f64,
    /// Normalized flow exposure \[0, 1\]. Higher values indicate stronger
    /// water circulation past the habitat surface.
    pub flow_exposure: f64,
    /// Normalized oxygen exposure \[0, 1\]. Higher values indicate better
    /// dissolved oxygen availability at the habitat.
    pub oxygen_exposure: f64,
    /// Normalized light exposure \[0, 1\]. Structural fraction of tank
    /// illumination that reaches the habitat surface.
    pub light_exposure: f64,
}

/// Shared environmental context computed once per registry build and passed
/// to each per-habitat helper.
struct HabitatEnv {
    normalized_flow: f64,
    aeration_boost: f64,
    light_intensity: f64,
    avg_crowding: f64,
    avg_low_o2: f64,
    root_zone_o2_boost: f64,
    /// Beer-Lambert: fraction of surface light reaching the substrate surface.
    substrate_light_fraction: f64,
    /// Beer-Lambert: mean fraction of surface light across the water column.
    column_avg_light_fraction: f64,
}

/// Recompute the full habitat registry from current tank state.
///
/// Colonizable areas are derived from geometry, hardware configuration, and
/// plant biomass. Exposure modifiers respond to filter flow, aeration,
/// clogging, light intensity, substrate properties, and rooted plant presence.
pub fn compute_habitat_registry(state: &TankState) -> Vec<HabitatEntry> {
    let volume_l = state.water_volume_l().max(f64::EPSILON);
    let flow_lph = if state.hardware.filter.enabled {
        state.hardware.filter.flow_lph
    } else {
        0.0
    };

    let has_rooted_plants = state
        .plant_guilds
        .iter()
        .any(|p| matches!(p.guild, PlantGuild::RootFeedingRosette) && p.biomass_g > 0.1);

    let k = state.extinction_coefficient();
    let h = state.water_depth_above_substrate_cm();

    let env = HabitatEnv {
        // Turnover rate normalized: 10 turnovers/hour maps to 1.0
        normalized_flow: (flow_lph / volume_l / 10.0).clamp(0.0, 1.0),
        aeration_boost: if state.hardware.aeration.enabled {
            state.hardware.aeration.intensity
        } else {
            0.0
        },
        light_intensity: if state.hardware.light.enabled {
            state.hardware.light.intensity_index
        } else {
            0.0
        },
        avg_crowding: state.derived_plant_crowding_index(),
        avg_low_o2: state.avg_substrate_index(|l| l.low_oxygen_tendency_index),
        root_zone_o2_boost: if has_rooted_plants { 0.1 } else { 0.0 },
        substrate_light_fraction: beer_lambert_at_depth(k, h),
        column_avg_light_fraction: column_average_attenuation_factor(k, h),
    };

    HabitatKind::ALL
        .iter()
        .map(|&kind| {
            let (area, flow, oxygen, light) = match kind {
                HabitatKind::FilterMedia => compute_filter_media(state, &env),
                HabitatKind::GlassHardscape => compute_glass_hardscape(state, &env),
                HabitatKind::PlantSurfaces => compute_plant_surfaces(state, &env),
                HabitatKind::SubstrateSurface => compute_substrate_surface(state, &env),
                HabitatKind::SubstrateDeep => compute_substrate_deep(state, &env),
            };
            HabitatEntry {
                kind,
                colonizable_area_cm2: area.max(0.0),
                flow_exposure: flow.clamp(0.0, 1.0),
                oxygen_exposure: oxygen.clamp(0.0, 1.0),
                light_exposure: light.clamp(0.0, 1.0),
            }
        })
        .collect()
}

/// Look up a single habitat entry by kind from a registry slice.
pub fn find_habitat(registry: &[HabitatEntry], kind: HabitatKind) -> Option<&HabitatEntry> {
    registry.iter().find(|h| h.kind == kind)
}

// ---------------------------------------------------------------------------
// Per-habitat computation helpers
// ---------------------------------------------------------------------------

fn compute_filter_media(state: &TankState, env: &HabitatEnv) -> (f64, f64, f64, f64) {
    let area = if state.hardware.filter.enabled {
        state.hardware.filter.media_area_cm2
    } else {
        0.0
    };

    let flow = if state.hardware.filter.enabled {
        0.6 + 0.35 * env.normalized_flow
    } else {
        0.05
    };

    let oxygen = if state.hardware.filter.enabled {
        (0.6 + 0.2 * env.aeration_boost) * (1.0 - 0.5 * state.filter_state.clogging_index)
    } else {
        0.1
    };

    let light = 0.05 * env.light_intensity;

    (area, flow, oxygen, light)
}

fn compute_glass_hardscape(state: &TankState, env: &HabitatEnv) -> (f64, f64, f64, f64) {
    let area = state.geometry.wall_area_cm2() + state.geometry.hardscape_area_cm2;

    let flow = 0.15 + 0.45 * env.normalized_flow;
    let oxygen = 0.6 + 0.25 * env.aeration_boost + 0.1 * env.normalized_flow;
    let light =
        0.4 * env.light_intensity * env.column_avg_light_fraction * (1.0 - 0.3 * env.avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_plant_surfaces(state: &TankState, env: &HabitatEnv) -> (f64, f64, f64, f64) {
    let area: f64 = state
        .plant_guilds
        .iter()
        .map(|p| {
            let sla = match p.guild {
                PlantGuild::FastStem => FAST_STEM_SPECIFIC_LEAF_AREA_CM2_PER_G,
                PlantGuild::RootFeedingRosette => ROOT_ROSETTE_SPECIFIC_LEAF_AREA_CM2_PER_G,
            };
            p.biomass_g * sla
        })
        .sum();

    let flow = 0.1 + 0.3 * env.normalized_flow;
    let oxygen = 0.65 + 0.2 * env.aeration_boost + 0.1 * env.normalized_flow;
    let light =
        0.7 * env.light_intensity * env.column_avg_light_fraction * (1.0 - 0.3 * env.avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_substrate_surface(state: &TankState, env: &HabitatEnv) -> (f64, f64, f64, f64) {
    let area = if state.substrate_depth_cm() <= f64::EPSILON {
        0.0
    } else {
        state.geometry.footprint_area_cm2()
    };

    let flow = 0.05 + 0.2 * env.normalized_flow;
    let oxygen = 0.3 + 0.15 * env.normalized_flow - 0.15 * env.avg_low_o2
        + 0.1 * env.aeration_boost
        + env.root_zone_o2_boost;
    let light =
        0.25 * env.light_intensity * env.substrate_light_fraction * (1.0 - 0.5 * env.avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_substrate_deep(state: &TankState, env: &HabitatEnv) -> (f64, f64, f64, f64) {
    let footprint_area_cm2 = state.geometry.footprint_area_cm2();
    let area: f64 = state
        .substrate_layers
        .iter()
        .map(|layer| layer.derived_colonizable_area_cm2(footprint_area_cm2))
        .sum();

    let flow = 0.02 + 0.05 * env.normalized_flow;
    // Capped at 0.2: even with flow, deep substrate remains oxygen-limited.
    let oxygen = (0.1 * (1.0 - env.avg_low_o2) + 0.05 * env.normalized_flow).clamp(0.02, 0.2);
    let light = 0.0;

    (area, flow, oxygen, light)
}
