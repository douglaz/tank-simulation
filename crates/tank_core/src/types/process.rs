use serde::{Deserialize, Serialize};

pub const LEGACY_KINETIC_REFERENCE_VOLUME_L: f64 = 20.0;
pub const LEGACY_KINETIC_REFERENCE_FOOTPRINT_M2: f64 = 0.1;

/// Stoichiometric alkalinity consumption for complete nitrification (NH₄⁺ → NO₃⁻).
///
/// 2 meq per 14.007 mg N ≈ 0.1428 meq/mg N. The full charge belongs to the
/// TAN-oxidation step (AOB + comammox); NOB (NO₂⁻ → NO₃⁻) does not consume
/// additional alkalinity.
///
/// Source: Stumm & Morgan, *Aquatic Chemistry*, 3rd ed.
pub const NITRIFICATION_ALK_MEQ_PER_MG_N: f64 = 2.0 / 14.007;

/// Atomic mass of nitrogen, used to convert TAN release into alkalinity return.
///
/// NH4+ excretion/respiration restores 1 meq alkalinity per 14.007 mg N.
pub const MG_N_PER_MEQ_AMMONIA: f64 = 14.007;

/// Stoichiometric alkalinity production during denitrification (NO₃⁻ → N₂).
///
/// 1 meq per 14.007 mg N ≈ 0.0714 meq/mg N. Approximately half the
/// nitrification consumption is returned when nitrate is fully reduced.
///
/// Used in the denitrification pathway (`nitrogen_cycle.rs`) to restore
/// alkalinity consumed during nitrification:
/// `alk_produced = n_denitrified_mg * DENITRIFICATION_ALK_MEQ_PER_MG_N`.
pub const DENITRIFICATION_ALK_MEQ_PER_MG_N: f64 = 1.0 / 14.007;

pub fn legacy_total_param_to_mg_per_l(value: f64) -> f64 {
    if value.is_finite() {
        (value / LEGACY_KINETIC_REFERENCE_VOLUME_L).max(0.0)
    } else {
        0.0
    }
}

pub fn legacy_total_param_to_mg_per_m2(value: f64) -> f64 {
    if value.is_finite() {
        (value / LEGACY_KINETIC_REFERENCE_FOOTPRINT_M2).max(0.0)
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
    /// Decomposer DOC half-saturation constant (mg C / L).
    /// Monod-style: limitation = [DOC] / ([DOC] + K_s).
    /// Literature range: 1–10 mg C/L.
    #[serde(alias = "decomposer_k_doc_mg")]
    pub decomposer_k_doc_mg_c_per_l: f64,
    /// Decomposer dissolved-oxygen half-saturation constant (mg O₂ / L).
    /// Literature range: 0.3–1.0 mg O₂/L.
    #[serde(alias = "decomposer_k_do_mg")]
    pub decomposer_k_do_mg_per_l: f64,
    /// Growth yield of decomposer biomass per g DOC consumed.
    pub decomposer_growth_yield: f64,
    /// Hourly decay rate of decomposer biomass under starvation.
    pub decomposer_decay_rate_per_hour: f64,

    // -- Nitrifier guild kinetics --
    /// AOB vmax: max mg N oxidized per g AOB biomass per hour.
    pub aob_vmax_mg_n_per_g_per_hour: f64,
    /// AOB total-ammonia-nitrogen half-saturation constant (mg N / L).
    /// Literature range: 0.5–2.0 mg N/L.
    #[serde(alias = "aob_k_tan_mg")]
    pub aob_k_tan_mg_n_per_l: f64,
    /// AOB dissolved-oxygen half-saturation constant (mg O₂ / L).
    /// Literature range: 0.3–1.0 mg O₂/L.
    #[serde(alias = "aob_k_do_mg")]
    pub aob_k_do_mg_per_l: f64,
    /// AOB growth yield (g biomass per mg N oxidized).
    pub aob_growth_yield: f64,
    /// AOB decay rate per hour.
    pub aob_decay_rate_per_hour: f64,

    /// NOB vmax: max mg N oxidized per g NOB biomass per hour.
    pub nob_vmax_mg_n_per_g_per_hour: f64,
    /// NOB nitrite half-saturation constant (mg N / L).
    /// Literature range: 0.2–1.0 mg N/L.
    #[serde(alias = "nob_k_nitrite_mg")]
    pub nob_k_nitrite_mg_n_per_l: f64,
    /// NOB dissolved-oxygen half-saturation constant (mg O₂ / L).
    /// NOB are more DO-sensitive than AOB; K_s(DO) >= AOB K_s(DO).
    /// Literature range: 0.5–1.5 mg O₂/L.
    #[serde(alias = "nob_k_do_mg")]
    pub nob_k_do_mg_per_l: f64,
    /// NOB growth yield (g biomass per mg N oxidized).
    pub nob_growth_yield: f64,
    /// NOB decay rate per hour.
    pub nob_decay_rate_per_hour: f64,

    /// Comammox vmax multiplier relative to AOB (must be < 1.0).
    pub comammox_vmax_fraction: f64,
    /// Comammox TAN half-saturation constant (mg N / L).
    /// Comammox has a lower K_s(TAN) than AOB (competitive advantage at low ammonia).
    /// Literature range: 0.05–0.5 mg N/L.
    #[serde(alias = "comammox_k_tan_mg")]
    pub comammox_k_tan_mg_n_per_l: f64,
    /// Comammox dissolved-oxygen half-saturation constant (mg O₂ / L).
    /// Literature range: 0.3–1.0 mg O₂/L.
    #[serde(alias = "comammox_k_do_mg")]
    pub comammox_k_do_mg_per_l: f64,
    /// Comammox growth yield (g biomass per mg N oxidized).
    pub comammox_growth_yield: f64,
    /// Comammox decay rate per hour.
    pub comammox_decay_rate_per_hour: f64,

    // -- Denitrification kinetics --
    /// Maximum denitrification rate (mg N per L suboxic pore water per hour).
    /// Represents the potential rate at saturating NO₃ and DOC concentrations
    /// in fully mature substrate. Literature range: 0.01–0.5 mg N/L/h for
    /// freshwater sediments.
    #[serde(default = "crate::types::process::default_denitrification_vmax_mg_n_per_l_per_hour")]
    pub denitrification_vmax_mg_n_per_l_per_hour: f64,

    /// Nitrate half-saturation constant for denitrification (mg N / L).
    /// Monod-style: limitation = [NO₃] / ([NO₃] + K_s).
    /// Literature range: 0.5–5.0 mg N/L for freshwater denitrifiers.
    #[serde(default = "crate::types::process::default_denitrification_k_no3_mg_n_per_l")]
    pub denitrification_k_no3_mg_n_per_l: f64,

    /// DOC half-saturation constant for denitrification (mg C / L).
    /// Monod-style: limitation = [DOC] / ([DOC] + K_s).
    /// Literature range: 1.0–10.0 mg C/L.
    #[serde(default = "crate::types::process::default_denitrification_k_doc_mg_c_per_l")]
    pub denitrification_k_doc_mg_c_per_l: f64,

    /// Pore-water mixing factor: fraction of water-column concentration that
    /// reaches suboxic pore water via diffusion and advection.
    /// Range: 0.0 (no mixing) to 1.0 (perfect equilibrium).
    /// Typical: 0.3–0.7 for aquarium substrate with moderate flow.
    #[serde(default = "crate::types::process::default_denitrification_pore_water_mixing_factor")]
    pub denitrification_pore_water_mixing_factor: f64,

    /// Time constant for denitrifier activity maturation (days).
    /// The denitrifier activity index ramps from near-zero to 1.0 over
    /// this many days. Reflects the establishment of an anaerobic microbial
    /// community in the suboxic zone.
    #[serde(default = "crate::types::process::default_denitrification_activity_maturation_days")]
    pub denitrification_activity_maturation_days: f64,

    // -- Root-zone oxygenation (radial oxygen loss) --
    /// Low-biomass O₂ penetration slope per gram of substrate-active root
    /// biomass (cm/g). Represents radial oxygen loss (ROL) from rooted plant
    /// roots into the surrounding substrate.
    ///
    /// The substrate system applies this slope inside a saturating depth cap:
    /// `bonus = depth * (1 - exp(-(active_root_biomass_g * rate) / depth))`.
    /// That preserves a linear cm/g interpretation at low biomass while
    /// preventing unrealistic penetration at extreme biomass.
    ///
    /// Literature: ROL varies widely (0.01–0.5 cm/g depending on species);
    /// 0.15 cm/g is a moderate default for mixed planted-tank rosettes.
    #[serde(default = "crate::types::process::default_rol_rate_cm_per_g")]
    pub rol_rate_cm_per_g: f64,

    // -- Biofilter carrying capacity --
    /// Base nitrifier density (g biomass / cm² colonizable area).
    /// Multiplied by habitat area, flow, and oxygen exposure to compute
    /// the biofilter carrying capacity for nitrifying bacteria.
    /// Default 2.5e-4 g/cm² ≈ 0.5 g on the default 2000 cm² filter media.
    #[serde(default = "default_nitrifier_base_density_g_per_cm2")]
    pub nitrifier_base_density_g_per_cm2: f64,

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
    /// Water-column nitrogen half-saturation constant (mg N / L).
    /// Monod-style: limitation = [N] / ([N] + Ks).
    pub plant_half_saturation_n_mg_n_per_l: f64,
    /// Water-column phosphorus half-saturation constant (mg P / L).
    pub plant_half_saturation_p_mg_p_per_l: f64,
    /// DIC half-saturation constant (mg C / L).
    pub plant_half_saturation_c_mg_c_per_l: f64,
    /// Substrate nitrogen half-saturation constant (mg N / m²).
    /// Rooted plants access substrate N via root uptake; this Ks applies
    /// to the areal nutrient density rather than volumetric concentration.
    pub plant_half_saturation_n_substrate_mg_n_per_m2: f64,
    /// Substrate phosphorus half-saturation constant (mg P / m²).
    pub plant_half_saturation_p_substrate_mg_p_per_m2: f64,
    pub plant_light_half_saturation: f64,
    pub plant_temp_optimum_c: f64,
    pub plant_temp_sigma_c: f64,
    pub plant_crowding_biomass_g_per_m2: f64,

    // -- Daily algae/periphyton growth --
    pub algae_max_growth_rate_per_day: f64,
    pub periphyton_max_growth_rate_per_day: f64,
    pub algae_respiration_fraction_per_day: f64,
    /// Monod half-saturation constant for dissolved inorganic nitrogen
    /// (TAN + NO3⁻) limitation of algae growth, in mg N / L.
    /// Algae growth reaches half its nutrient-unlimited rate at this
    /// concentration.  Typical freshwater range: 0.05–0.5 mg N/L.
    #[serde(alias = "algae_half_saturation_n_mg_total")]
    pub algae_half_saturation_n_mg_n_per_l: f64,
    /// Monod half-saturation constant for dissolved phosphorus (PO₄³⁻)
    /// limitation of algae growth, in mg P / L.
    /// Typical freshwater range: 0.005–0.05 mg P/L.
    #[serde(alias = "algae_half_saturation_p_mg_total")]
    pub algae_half_saturation_p_mg_p_per_l: f64,
    pub algae_light_half_saturation: f64,
    pub algae_temp_optimum_c: f64,
    pub algae_temp_sigma_c: f64,
    pub periphyton_capacity_g_per_m2: f64,
    pub algae_bloom_threshold_g_per_l: f64,
    pub algae_nuisance_biomass_g_per_m2: f64,

    // -- Light attenuation (Beer-Lambert) --
    /// Base extinction coefficient for pure water (1/cm).
    /// Pure freshwater PAR: ~0.04/m = 0.0004/cm.
    #[serde(default = "default_base_extinction_coeff_per_cm")]
    pub base_extinction_coeff_per_cm: f64,
    /// Specific extinction coefficient for suspended algae (1/cm per g/L).
    #[serde(default = "default_algae_extinction_coeff")]
    pub algae_extinction_coeff_per_cm_per_g_l: f64,
    /// Specific extinction coefficient for dissolved organic carbon (1/cm per mg C/L).
    #[serde(default = "default_doc_extinction_coeff")]
    pub doc_extinction_coeff_per_cm_per_mg_c_l: f64,
    /// Specific extinction coefficient for fine detritus (1/cm per g/L).
    #[serde(default = "default_detritus_extinction_coeff")]
    pub detritus_extinction_coeff_per_cm_per_g_l: f64,

    // -- Shrimp dynamics --
    pub shrimp_base_mortality_per_day: f64,
    pub shrimp_stress_mortality_scale: f64,
    pub shrimp_juvenile_maturation_days: f64,
    pub shrimp_periphyton_grazing_g_per_shrimp_per_day: f64,
    pub shrimp_condition_smoothing: f64,

    // -- Shrimp feeding pathway fractions (consumer routing contract) --
    // Phase-1 simplification: one shared food-quality assumption for periphyton
    // and fine detritus. Both are treated as generic organic matter with the
    // same N:C ratio (feed_n_to_c_ratio). This is acceptable because
    // periphyton and fine detritus in a shrimp tank have broadly similar
    // elemental composition at the resolution of this model.
    //
    // Routing contract:
    //   ingested = feces + assimilated
    //   assimilated = respired + excreted + retained
    //   fecal_fraction = 1.0 - assimilation_efficiency
    //   respiration_fraction + excretion_fraction + growth_fraction = 1.0
    //
    // See docs/ROUTING.md for the full consumer routing contract.
    /// Fraction of ingested organic matter that is assimilated (remainder is feces).
    /// Literature range for detritivorous shrimp: 0.4–0.7.
    #[serde(default = "default_shrimp_assimilation_efficiency")]
    pub shrimp_assimilation_efficiency: f64,

    /// Of the assimilated share, fraction routed to respiration (O2 demand + DIC).
    /// Literature range: 0.6–0.8.
    #[serde(default = "default_shrimp_respiration_fraction")]
    pub shrimp_respiration_fraction_of_assimilated: f64,

    /// Of the assimilated share, fraction excreted as dissolved TAN.
    /// Literature range: 0.1–0.2.
    #[serde(default = "default_shrimp_excretion_fraction")]
    pub shrimp_excretion_fraction_of_assimilated: f64,

    /// Of the assimilated share, fraction retained as body reserve/growth.
    /// Derived: 1.0 - respiration_fraction - excretion_fraction.
    /// Literature range: 0.1–0.2.
    #[serde(default = "default_shrimp_growth_fraction")]
    pub shrimp_growth_fraction_of_assimilated: f64,

    /// Respiratory quotient: mg O2 consumed per mg C respired.
    /// Stoichiometric value for carbohydrate oxidation ≈ 2.67 (32/12).
    #[serde(default = "default_shrimp_o2_per_mg_c_respired")]
    pub shrimp_o2_per_mg_c_respired: f64,

    // -- Death routing --
    /// Fraction of dead organism biomass that enters fine_detritus_g_total.
    /// Phase 1 keeps this fixed at 1.0 until the simulation has an explicit
    /// export path for removed carcasses/reserve; lower values would silently
    /// delete tracked N/C from the closed-system budget. The field remains
    /// serialized so a later export-aware bead can relax the invariant without
    /// another schema change. See docs/ROUTING.md §Death routing.
    #[serde(default = "default_death_biomass_to_detritus_fraction")]
    pub death_biomass_to_detritus_fraction: f64,

    // -- Microfauna turnover --
    pub microfauna_mineralization_boost: f64,
    pub microfauna_periphyton_consumption: f64,
    pub microfauna_population_smoothing: f64,
    pub microfauna_shrimp_pressure_threshold: f64,

    // -- Microfauna feeding pathway fractions (consumer routing contract) --
    // Same routing template as shrimp: ingested = feces + assimilated,
    // assimilated = respired + excreted + retained.
    // Microfauna are index-based so the retained share accumulates in a
    // lightweight reserve_g pool rather than discrete biomass.
    /// Fraction of ingested organic matter that is assimilated (remainder is feces).
    #[serde(default = "default_microfauna_assimilation_efficiency")]
    pub microfauna_assimilation_efficiency: f64,

    /// Of the assimilated share, fraction routed to respiration (O2 demand + DIC).
    #[serde(default = "default_microfauna_respiration_fraction")]
    pub microfauna_respiration_fraction_of_assimilated: f64,

    /// Of the assimilated share, fraction excreted as dissolved TAN + DOC.
    #[serde(default = "default_microfauna_excretion_fraction")]
    pub microfauna_excretion_fraction_of_assimilated: f64,

    /// Of the assimilated share, fraction retained as microfauna reserve.
    /// Derived: 1.0 - respiration_fraction - excretion_fraction.
    #[serde(default = "default_microfauna_growth_fraction")]
    pub microfauna_growth_fraction_of_assimilated: f64,
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
            // Stoichiometrically consistent with O2 rates: C:O2 = 12:32 = 0.375.
            // respiration_dic = background_bod (0.05) × 12/32 = 0.01875
            // photosynthesis_dic = plant_photosynthesis_o2 (0.2) × 12/32 = 0.075
            respiration_dic_rate_mg_c_per_g_per_hour: 0.01875,
            photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.075,
            k_surface_w_per_m2_k: 10.0,
            k_wall_w_per_m2_k: 5.0,

            feed_leach_rate_per_hour: 0.12,
            fine_detritus_dissolution_rate_per_hour: 0.08,
            feed_n_to_c_ratio: 0.16,

            decomposer_vmax_per_hour: 0.02,
            decomposer_k_doc_mg_c_per_l: 3.0,
            decomposer_k_do_mg_per_l: 0.5,
            decomposer_growth_yield: 0.3,
            decomposer_decay_rate_per_hour: 0.002,

            aob_vmax_mg_n_per_g_per_hour: 1.5,
            aob_k_tan_mg_n_per_l: 1.0,
            aob_k_do_mg_per_l: 0.5,
            aob_growth_yield: 0.05,
            aob_decay_rate_per_hour: 0.003,

            nob_vmax_mg_n_per_g_per_hour: 1.2,
            nob_k_nitrite_mg_n_per_l: 0.5,
            nob_k_do_mg_per_l: 0.8,
            nob_growth_yield: 0.04,
            nob_decay_rate_per_hour: 0.003,

            comammox_vmax_fraction: 0.4,
            comammox_k_tan_mg_n_per_l: 0.2,
            comammox_k_do_mg_per_l: 0.6,
            comammox_growth_yield: 0.03,
            comammox_decay_rate_per_hour: 0.004,

            denitrification_vmax_mg_n_per_l_per_hour:
                crate::types::process::default_denitrification_vmax_mg_n_per_l_per_hour(),
            denitrification_k_no3_mg_n_per_l:
                crate::types::process::default_denitrification_k_no3_mg_n_per_l(),
            denitrification_k_doc_mg_c_per_l:
                crate::types::process::default_denitrification_k_doc_mg_c_per_l(),
            denitrification_pore_water_mixing_factor:
                crate::types::process::default_denitrification_pore_water_mixing_factor(),
            denitrification_activity_maturation_days:
                crate::types::process::default_denitrification_activity_maturation_days(),

            rol_rate_cm_per_g: crate::types::process::default_rol_rate_cm_per_g(),

            nitrifier_base_density_g_per_cm2: default_nitrifier_base_density_g_per_cm2(),

            o2_per_mg_n_nitrified: 4.57,
            alkalinity_meq_per_mg_n_nitrified: NITRIFICATION_ALK_MEQ_PER_MG_N,

            plant_max_growth_rate_fast_stem_per_day: 0.08,
            plant_max_growth_rate_root_rosette_per_day: 0.06,
            plant_respiration_fraction_per_day: 0.01,
            plant_senescence_fraction_per_day: 0.005,
            plant_health_recovery_per_day: 0.03,
            plant_health_decline_per_day: 0.08,
            plant_half_saturation_n_mg_n_per_l: 0.4,
            plant_half_saturation_p_mg_p_per_l: 0.06,
            plant_half_saturation_c_mg_c_per_l: 1.0,
            plant_half_saturation_n_substrate_mg_n_per_m2: 80.0,
            plant_half_saturation_p_substrate_mg_p_per_m2: 12.0,
            plant_light_half_saturation: 0.45,
            plant_temp_optimum_c: 25.0,
            plant_temp_sigma_c: 7.0,
            plant_crowding_biomass_g_per_m2: 250.0,

            algae_max_growth_rate_per_day: 0.2,
            periphyton_max_growth_rate_per_day: 0.15,
            algae_respiration_fraction_per_day: 0.03,
            algae_half_saturation_n_mg_n_per_l: 0.25,
            algae_half_saturation_p_mg_p_per_l: 0.04,
            algae_light_half_saturation: 0.35,
            algae_temp_optimum_c: 27.0,
            algae_temp_sigma_c: 8.0,
            periphyton_capacity_g_per_m2: 6.0,
            algae_bloom_threshold_g_per_l: 0.08,
            algae_nuisance_biomass_g_per_m2: 10.0,

            base_extinction_coeff_per_cm: default_base_extinction_coeff_per_cm(),
            algae_extinction_coeff_per_cm_per_g_l: default_algae_extinction_coeff(),
            doc_extinction_coeff_per_cm_per_mg_c_l: default_doc_extinction_coeff(),
            detritus_extinction_coeff_per_cm_per_g_l: default_detritus_extinction_coeff(),

            shrimp_base_mortality_per_day: 0.002,
            shrimp_stress_mortality_scale: 0.15,
            shrimp_juvenile_maturation_days: 30.0,
            shrimp_periphyton_grazing_g_per_shrimp_per_day: 0.01,
            shrimp_condition_smoothing: 0.15,

            shrimp_assimilation_efficiency: default_shrimp_assimilation_efficiency(),
            shrimp_respiration_fraction_of_assimilated: default_shrimp_respiration_fraction(),
            shrimp_excretion_fraction_of_assimilated: default_shrimp_excretion_fraction(),
            shrimp_growth_fraction_of_assimilated: default_shrimp_growth_fraction(),
            shrimp_o2_per_mg_c_respired: default_shrimp_o2_per_mg_c_respired(),

            death_biomass_to_detritus_fraction: default_death_biomass_to_detritus_fraction(),

            microfauna_mineralization_boost: 0.15,
            microfauna_periphyton_consumption: 0.02,
            microfauna_population_smoothing: 0.1,
            microfauna_shrimp_pressure_threshold: 3.0,

            microfauna_assimilation_efficiency: default_microfauna_assimilation_efficiency(),
            microfauna_respiration_fraction_of_assimilated: default_microfauna_respiration_fraction(
            ),
            microfauna_excretion_fraction_of_assimilated: default_microfauna_excretion_fraction(),
            microfauna_growth_fraction_of_assimilated: default_microfauna_growth_fraction(),
        }
    }
}

// -- Shrimp feeding pathway defaults --
// Phase-1 values chosen from literature on Neocaridina/Caridina detritivory.
// respiration (0.70) + excretion (0.10) + growth (0.20) = 1.0

fn default_shrimp_assimilation_efficiency() -> f64 {
    0.50
}
fn default_shrimp_respiration_fraction() -> f64 {
    0.70
}
fn default_shrimp_excretion_fraction() -> f64 {
    0.10
}
fn default_shrimp_growth_fraction() -> f64 {
    0.20
}
/// Stoichiometric O2:C for organic matter oxidation (32/12 ≈ 2.67).
fn default_shrimp_o2_per_mg_c_respired() -> f64 {
    2.67
}

/// All dead biomass stays in-tank as fine detritus by default.
fn default_death_biomass_to_detritus_fraction() -> f64 {
    1.0
}

// -- Denitrification defaults --

/// Conservative vmax for freshwater aquarium substrate denitrification.
/// Literature values range 0.01–0.5 mg N/L/h; 0.1 sits mid-range.
pub(crate) fn default_denitrification_vmax_mg_n_per_l_per_hour() -> f64 {
    0.1
}

/// NO₃ half-saturation for denitrification: 2.0 mg N/L.
/// Literature range: 0.5–5.0 mg N/L.
pub(crate) fn default_denitrification_k_no3_mg_n_per_l() -> f64 {
    2.0
}

/// DOC half-saturation for denitrification: 5.0 mg C/L.
/// Literature range: 1.0–10.0 mg C/L.
pub(crate) fn default_denitrification_k_doc_mg_c_per_l() -> f64 {
    5.0
}

/// Fraction of water-column concentration reaching suboxic pore water.
/// Moderate mixing: 0.5 (50% of overlying concentration).
pub(crate) fn default_denitrification_pore_water_mixing_factor() -> f64 {
    0.5
}

/// Days for denitrifier community to reach full activity.
/// Anaerobic communities establish slowly: ~60 days typical for aquaria.
pub(crate) fn default_denitrification_activity_maturation_days() -> f64 {
    60.0
}

// -- Root-zone oxygenation (radial oxygen loss) defaults --

/// Default ROL rate: 0.15 cm per gram of substrate-active root biomass at the
/// low-biomass slope. The substrate system applies an exponential saturation
/// against the finite bed depth.
pub(crate) fn default_rol_rate_cm_per_g() -> f64 {
    0.15
}

// -- Biofilter carrying capacity defaults --

/// Default nitrifier density: 2.5e-4 g/cm². With the default 2000 cm²
/// filter media at full flow and O2 exposure this yields ~0.5 g capacity,
/// preserving backward compatibility with the former hardcoded constant.
fn default_nitrifier_base_density_g_per_cm2() -> f64 {
    2.5e-4
}

// -- Microfauna feeding pathway defaults --
// Phase-1 values parallel shrimp defaults. Microfauna are smaller organisms
// with similar detritivorous feeding ecology.
// respiration (0.70) + excretion (0.10) + growth (0.20) = 1.0

fn default_microfauna_assimilation_efficiency() -> f64 {
    0.50
}
fn default_microfauna_respiration_fraction() -> f64 {
    0.70
}
fn default_microfauna_excretion_fraction() -> f64 {
    0.10
}
fn default_microfauna_growth_fraction() -> f64 {
    0.20
}

// -- Light attenuation defaults (Beer-Lambert) --

/// Pure freshwater PAR extinction: ~0.04/m = 0.0004/cm.
fn default_base_extinction_coeff_per_cm() -> f64 {
    0.0004
}
/// Suspended algae specific extinction. Typical range 1–4 /cm per g/L
/// for green microalgae at PAR wavelengths.
fn default_algae_extinction_coeff() -> f64 {
    2.0
}
/// Dissolved organic carbon (humic/tannin) specific extinction.
/// Typical range 0.001–0.005 /cm per mg C/L for freshwater DOC.
fn default_doc_extinction_coeff() -> f64 {
    0.002
}
/// Fine detritus (suspended particulates) specific extinction.
fn default_detritus_extinction_coeff() -> f64 {
    0.5
}
