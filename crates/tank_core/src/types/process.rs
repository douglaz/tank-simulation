use serde::{Deserialize, Serialize};

pub const LEGACY_KINETIC_REFERENCE_VOLUME_L: f64 = 20.0;

pub fn legacy_total_param_to_mg_per_l(value: f64) -> f64 {
    if value.is_finite() {
        (value / LEGACY_KINETIC_REFERENCE_VOLUME_L).max(0.0)
    } else {
        0.0
    }
}

/// Runtime process parameters for heat-transfer coefficients and later chemistry systems.
/// Stored in `TankState` for deterministic continuation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProcessParams {
    /// Reserved: not yet wired into the nitrogen cycle. The decomposer and
    /// nitrifier guild rates (`decomposer_vmax_per_hour`, `aob_vmax_*`, etc.)
    /// are the effective controls. Changing this field has no effect.
    pub mineralization_rate_per_day: f64,
    /// Reserved: not yet wired into the nitrogen cycle. See per-guild vmax
    /// fields for the effective nitrification rate controls.
    pub nitrification_vmax: f64,
    pub reaeration_kla_base: f64,
    pub aeration_kla_boost: f64,
    pub background_bod_mg_o2_per_g_biomass_per_hour: f64,
    pub plant_photosynthesis_o2_mg_per_g_per_hour: f64,
    pub respiration_dic_rate_mg_c_per_g_per_hour: f64,
    pub photosynthesis_dic_rate_mg_c_per_g_per_hour: f64,
    /// Surface (top) heat-transfer coefficient in W/(m²·K).
    pub k_surface_w_per_m2_k: f64,
    /// Wall heat-transfer coefficient in W/(m²·K).
    pub k_wall_w_per_m2_k: f64,

    // -- Nitrogen cycle: feed leaching --
    /// Fraction of particulate organics that leach to fine detritus per hour.
    pub feed_leach_rate_per_hour: f64,
    /// Fraction of fine detritus that dissolves to DOC/DON per hour.
    pub fine_detritus_dissolution_rate_per_hour: f64,
    /// N:C mass ratio in feed/detritus (mg N per mg C).
    pub feed_n_to_c_ratio: f64,

    // -- Decomposer mineralization --
    /// Max mineralization rate per g decomposer biomass per hour (g DOC consumed).
    pub decomposer_vmax_per_hour: f64,
    /// Legacy compatibility name for the decomposer DOC half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub decomposer_k_doc_mg: f64,
    /// Legacy compatibility name for the decomposer DO half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub decomposer_k_do_mg: f64,
    /// Growth yield of decomposer biomass per g DOC consumed.
    pub decomposer_growth_yield: f64,
    /// Hourly decay rate of decomposer biomass under starvation.
    pub decomposer_decay_rate_per_hour: f64,

    // -- Nitrifier guild kinetics --
    /// AOB vmax: max mg N oxidized per g AOB biomass per hour.
    pub aob_vmax_mg_n_per_g_per_hour: f64,
    /// Legacy compatibility name for the AOB TAN half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub aob_k_tan_mg: f64,
    /// Legacy compatibility name for the AOB DO half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub aob_k_do_mg: f64,
    /// AOB growth yield (g biomass per mg N oxidized).
    pub aob_growth_yield: f64,
    /// AOB decay rate per hour.
    pub aob_decay_rate_per_hour: f64,

    /// NOB vmax: max mg N oxidized per g NOB biomass per hour.
    pub nob_vmax_mg_n_per_g_per_hour: f64,
    /// Legacy compatibility name for the NOB nitrite half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub nob_k_nitrite_mg: f64,
    /// Legacy compatibility name for the NOB DO half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub nob_k_do_mg: f64,
    /// NOB growth yield (g biomass per mg N oxidized).
    pub nob_growth_yield: f64,
    /// NOB decay rate per hour.
    pub nob_decay_rate_per_hour: f64,

    /// Comammox vmax multiplier relative to AOB (must be < 1.0).
    pub comammox_vmax_fraction: f64,
    /// Legacy compatibility name for the comammox TAN half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub comammox_k_tan_mg: f64,
    /// Legacy compatibility name for the comammox DO half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective Monod constant stays concentration-based.
    pub comammox_k_do_mg: f64,
    /// Comammox growth yield (g biomass per mg N oxidized).
    pub comammox_growth_yield: f64,
    /// Comammox decay rate per hour.
    pub comammox_decay_rate_per_hour: f64,

    // -- Stoichiometric constants --
    /// mg O2 consumed per mg N fully nitrified to nitrate.
    pub o2_per_mg_n_nitrified: f64,
    /// meq alkalinity consumed per mg N nitrified.
    pub alkalinity_meq_per_mg_n_nitrified: f64,

    // -- Daily plant growth --
    pub plant_max_growth_rate_fast_stem_per_day: f64,
    pub plant_max_growth_rate_root_rosette_per_day: f64,
    pub plant_respiration_fraction_per_day: f64,
    pub plant_senescence_fraction_per_day: f64,
    pub plant_health_recovery_per_day: f64,
    pub plant_health_decline_per_day: f64,
    /// Legacy compatibility name for the water-column N half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective limitation stays concentration-based.
    pub plant_half_saturation_n_mg_total: f64,
    /// Legacy compatibility name for the water-column P half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective limitation stays concentration-based.
    pub plant_half_saturation_p_mg_total: f64,
    /// Legacy compatibility name for the DIC half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective limitation stays concentration-based.
    pub plant_half_saturation_c_mg_total: f64,
    pub plant_light_half_saturation: f64,
    pub plant_temp_optimum_c: f64,
    pub plant_temp_sigma_c: f64,
    pub plant_crowding_biomass_g_per_m2: f64,

    // -- Daily algae/periphyton growth --
    pub algae_max_growth_rate_per_day: f64,
    pub periphyton_max_growth_rate_per_day: f64,
    pub algae_respiration_fraction_per_day: f64,
    /// Legacy compatibility name for the algae water-column N half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective limitation stays concentration-based.
    pub algae_half_saturation_n_mg_total: f64,
    /// Legacy compatibility name for the algae water-column P half-saturation.
    /// Runtime code normalizes this total-style value onto a 20 L reference
    /// tank so the effective limitation stays concentration-based.
    pub algae_half_saturation_p_mg_total: f64,
    pub algae_light_half_saturation: f64,
    pub algae_temp_optimum_c: f64,
    pub algae_temp_sigma_c: f64,
    pub periphyton_capacity_g_per_m2: f64,
    pub algae_bloom_threshold_g_per_l: f64,
    pub algae_nuisance_biomass_g_per_m2: f64,

    // -- Shrimp dynamics --
    pub shrimp_base_mortality_per_day: f64,
    pub shrimp_stress_mortality_scale: f64,
    pub shrimp_juvenile_maturation_days: f64,
    pub shrimp_periphyton_grazing_g_per_shrimp_per_day: f64,
    pub shrimp_condition_smoothing: f64,

    // -- Microfauna turnover --
    pub microfauna_mineralization_boost: f64,
    pub microfauna_periphyton_consumption: f64,
    pub microfauna_population_smoothing: f64,
    pub microfauna_shrimp_pressure_threshold: f64,
}

impl Default for ProcessParams {
    fn default() -> Self {
        Self {
            mineralization_rate_per_day: 0.15,
            nitrification_vmax: 0.08,
            reaeration_kla_base: 0.35,
            aeration_kla_boost: 0.9,
            background_bod_mg_o2_per_g_biomass_per_hour: 0.05,
            plant_photosynthesis_o2_mg_per_g_per_hour: 0.2,
            respiration_dic_rate_mg_c_per_g_per_hour: 0.0,
            photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.0,
            k_surface_w_per_m2_k: 10.0,
            k_wall_w_per_m2_k: 5.0,

            feed_leach_rate_per_hour: 0.12,
            fine_detritus_dissolution_rate_per_hour: 0.08,
            feed_n_to_c_ratio: 0.16,

            decomposer_vmax_per_hour: 0.02,
            decomposer_k_doc_mg: 5.0,
            decomposer_k_do_mg: 2.0,
            decomposer_growth_yield: 0.3,
            decomposer_decay_rate_per_hour: 0.002,

            aob_vmax_mg_n_per_g_per_hour: 1.5,
            aob_k_tan_mg: 0.5,
            aob_k_do_mg: 1.0,
            aob_growth_yield: 0.05,
            aob_decay_rate_per_hour: 0.003,

            nob_vmax_mg_n_per_g_per_hour: 1.2,
            nob_k_nitrite_mg: 0.3,
            nob_k_do_mg: 1.0,
            nob_growth_yield: 0.04,
            nob_decay_rate_per_hour: 0.003,

            comammox_vmax_fraction: 0.4,
            comammox_k_tan_mg: 0.8,
            comammox_k_do_mg: 1.5,
            comammox_growth_yield: 0.03,
            comammox_decay_rate_per_hour: 0.004,

            o2_per_mg_n_nitrified: 4.57,
            alkalinity_meq_per_mg_n_nitrified: 0.1428,

            plant_max_growth_rate_fast_stem_per_day: 0.08,
            plant_max_growth_rate_root_rosette_per_day: 0.06,
            plant_respiration_fraction_per_day: 0.01,
            plant_senescence_fraction_per_day: 0.005,
            plant_health_recovery_per_day: 0.03,
            plant_health_decline_per_day: 0.08,
            plant_half_saturation_n_mg_total: 8.0,
            plant_half_saturation_p_mg_total: 1.2,
            plant_half_saturation_c_mg_total: 20.0,
            plant_light_half_saturation: 0.45,
            plant_temp_optimum_c: 25.0,
            plant_temp_sigma_c: 7.0,
            plant_crowding_biomass_g_per_m2: 250.0,

            algae_max_growth_rate_per_day: 0.2,
            periphyton_max_growth_rate_per_day: 0.15,
            algae_respiration_fraction_per_day: 0.03,
            algae_half_saturation_n_mg_total: 5.0,
            algae_half_saturation_p_mg_total: 0.8,
            algae_light_half_saturation: 0.35,
            algae_temp_optimum_c: 27.0,
            algae_temp_sigma_c: 8.0,
            periphyton_capacity_g_per_m2: 6.0,
            algae_bloom_threshold_g_per_l: 0.08,
            algae_nuisance_biomass_g_per_m2: 10.0,

            shrimp_base_mortality_per_day: 0.002,
            shrimp_stress_mortality_scale: 0.15,
            shrimp_juvenile_maturation_days: 30.0,
            shrimp_periphyton_grazing_g_per_shrimp_per_day: 0.01,
            shrimp_condition_smoothing: 0.15,

            microfauna_mineralization_boost: 0.15,
            microfauna_periphyton_consumption: 0.02,
            microfauna_population_smoothing: 0.1,
            microfauna_shrimp_pressure_threshold: 3.0,
        }
    }
}
