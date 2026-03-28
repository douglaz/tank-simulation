use serde::{Deserialize, Serialize};

use super::{PlantGuild, TankState};

/// Specific leaf area for FastStem guild (cm² of colonizable surface per gram).
const FAST_STEM_SPECIFIC_LEAF_AREA_CM2_PER_G: f64 = 40.0;

/// Specific leaf area for RootFeedingRosette guild (cm² per gram).
const ROOT_ROSETTE_SPECIFIC_LEAF_AREA_CM2_PER_G: f64 = 25.0;

/// Ecological zones where biofilm can colonize, decomposition can occur,
/// or redox conditions differ from the bulk water column.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    // Turnover rate normalized: 10 turnovers/hour maps to 1.0
    let normalized_flow = (flow_lph / volume_l / 10.0).clamp(0.0, 1.0);

    let aeration_boost = if state.hardware.aeration.enabled {
        state.hardware.aeration.intensity
    } else {
        0.0
    };

    let light_intensity = if state.hardware.light.enabled {
        state.hardware.light.intensity_index
    } else {
        0.0
    };

    let avg_crowding = if state.plant_guilds.is_empty() {
        0.0
    } else {
        state
            .plant_guilds
            .iter()
            .map(|p| p.crowding_index)
            .sum::<f64>()
            / state.plant_guilds.len() as f64
    };

    let avg_low_o2 = state.avg_substrate_index(|l| l.low_oxygen_tendency_index);

    let has_rooted_plants = state
        .plant_guilds
        .iter()
        .any(|p| matches!(p.guild, PlantGuild::RootFeedingRosette) && p.biomass_g > 0.1);
    let root_zone_o2_boost = if has_rooted_plants { 0.1 } else { 0.0 };

    let depth_attenuation = (1.0 - state.geometry.mean_depth_cm() / 60.0).clamp(0.1, 1.0);

    HabitatKind::ALL
        .iter()
        .map(|&kind| {
            let (area, flow, oxygen, light) = match kind {
                HabitatKind::FilterMedia => {
                    compute_filter_media(state, normalized_flow, aeration_boost, light_intensity)
                }
                HabitatKind::GlassHardscape => compute_glass_hardscape(
                    state,
                    normalized_flow,
                    aeration_boost,
                    light_intensity,
                    avg_crowding,
                ),
                HabitatKind::PlantSurfaces => compute_plant_surfaces(
                    state,
                    normalized_flow,
                    aeration_boost,
                    light_intensity,
                    avg_crowding,
                ),
                HabitatKind::SubstrateSurface => compute_substrate_surface(
                    state,
                    normalized_flow,
                    aeration_boost,
                    light_intensity,
                    avg_crowding,
                    avg_low_o2,
                    root_zone_o2_boost,
                    depth_attenuation,
                ),
                HabitatKind::SubstrateDeep => {
                    compute_substrate_deep(state, normalized_flow, avg_low_o2)
                }
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

fn compute_filter_media(
    state: &TankState,
    normalized_flow: f64,
    aeration_boost: f64,
    light_intensity: f64,
) -> (f64, f64, f64, f64) {
    let area = if state.hardware.filter.enabled {
        state.hardware.filter.media_area_cm2
    } else {
        0.0
    };

    let flow = if state.hardware.filter.enabled {
        0.6 + 0.35 * normalized_flow
    } else {
        0.05
    };

    let oxygen = if state.hardware.filter.enabled {
        (0.6 + 0.2 * aeration_boost) * (1.0 - 0.5 * state.filter_state.clogging_index)
    } else {
        0.1
    };

    let light = 0.05 * light_intensity;

    (area, flow, oxygen, light)
}

fn compute_glass_hardscape(
    state: &TankState,
    normalized_flow: f64,
    aeration_boost: f64,
    light_intensity: f64,
    avg_crowding: f64,
) -> (f64, f64, f64, f64) {
    let area = state.geometry.wall_area_cm2() + state.geometry.hardscape_area_cm2;

    let flow = 0.15 + 0.45 * normalized_flow;
    let oxygen = 0.6 + 0.25 * aeration_boost + 0.1 * normalized_flow;
    let light = 0.4 * light_intensity * (1.0 - 0.3 * avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_plant_surfaces(
    state: &TankState,
    normalized_flow: f64,
    aeration_boost: f64,
    light_intensity: f64,
    avg_crowding: f64,
) -> (f64, f64, f64, f64) {
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

    let flow = 0.1 + 0.3 * normalized_flow;
    let oxygen = 0.65 + 0.2 * aeration_boost + 0.1 * normalized_flow;
    let light = 0.7 * light_intensity * (1.0 - 0.3 * avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_substrate_surface(
    state: &TankState,
    normalized_flow: f64,
    aeration_boost: f64,
    light_intensity: f64,
    avg_crowding: f64,
    avg_low_o2: f64,
    root_zone_o2_boost: f64,
    depth_attenuation: f64,
) -> (f64, f64, f64, f64) {
    let area = if state.substrate_layers.is_empty() {
        0.0
    } else {
        state.geometry.footprint_area_cm2()
    };

    let flow = 0.05 + 0.2 * normalized_flow;
    let oxygen = 0.3 + 0.15 * normalized_flow - 0.15 * avg_low_o2 + 0.1 * aeration_boost
        + root_zone_o2_boost;
    let light = 0.25 * light_intensity * depth_attenuation * (1.0 - 0.5 * avg_crowding);

    (area, flow, oxygen, light)
}

fn compute_substrate_deep(
    state: &TankState,
    normalized_flow: f64,
    avg_low_o2: f64,
) -> (f64, f64, f64, f64) {
    let area: f64 = state
        .substrate_layers
        .iter()
        .map(|l| l.colonizable_area_cm2)
        .sum();

    let flow = 0.02 + 0.05 * normalized_flow;
    // Capped at 0.2: even with flow, deep substrate remains oxygen-limited.
    let oxygen = (0.1 * (1.0 - avg_low_o2) + 0.05 * normalized_flow).clamp(0.02, 0.2);
    let light = 0.0;

    (area, flow, oxygen, light)
}
