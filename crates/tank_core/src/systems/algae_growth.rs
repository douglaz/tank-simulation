use crate::{
    systems::chemistry::resolve_carbonate_state,
    systems::events,
    types::{
        algae_carbon_mg, algae_detrital_mass_g, total_colonizable_area_cm2, TankState,
        ALGAE_N_MG_PER_G_BIOMASS,
    },
};

const ALGAE_P_MG_PER_G_GROWTH: f64 = 5.0;

/// Daily algae growth step for both suspended (planktonic) and periphyton
/// (surface-attached) pools.
///
/// # Interactions modelled
///
/// Growth is the product of independent limitation factors (Monod kinetics
/// with Liebig's Law of the Minimum for nutrients):
///
/// - **Light**: combined photoperiod duration and hardware intensity through
///   a half-saturation curve.  Plants compete for the same light via a
///   shading term that reduces available light as crowding increases.
/// - **Nutrients (N, P)**: dissolved inorganic nitrogen (TAN + NO₃⁻) and
///   phosphorus (PO₄³⁻) each produce a Monod limitation factor; the
///   minimum of the two applies (Liebig's Law).  All half-saturation
///   constants are expressed in mg-element / L (concentration-based).
/// - **Temperature**: a Gaussian response centred on an optimum, with a
///   configurable sigma controlling the breadth of the thermal window.
/// - **Carbon**: DIC availability caps realised growth after gross growth
///   is computed (post-hoc nutrient cap), ensuring carbon mass balance.
///
/// # Light attenuation
///
/// Light available to algae is attenuated by depth and turbidity using
/// Beer-Lambert: I(z) = I₀ × exp(−k×z).  The extinction coefficient k
/// combines pure-water PAR absorption, suspended algae self-shading,
/// dissolved organic carbon (tannins), and fine detritus.  Suspended
/// algae use the column-average PAR; this creates a self-shading feedback
/// where denser blooms reduce their own light supply.
///
/// # Intentionally abstracted
///
/// - Planktonic and periphyton algae share the same nutrient/light/temperature
///   limitation factors.  Phase 3 (tanksim-6e5.5.3) will split them with
///   habitat-specific light exposure and nutrient access.
/// - P cycling is not fully closed; the model over-indexes on N limitation.
///   This is acceptable until the P cycle is closed in a later phase.
/// - CO₂/DIC interaction with photosynthesis and pH is deferred to
///   tanksim-6e5.4.4.
pub fn step_daily_algae(state: &mut TankState) {
    let concentrations = state.concentrations();
    let volume_l = concentrations.volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let hourly_photosynthesis_dic_enabled =
        state.process_params.photosynthesis_dic_rate_mg_c_per_g_per_hour > f64::EPSILON;
    let tan_mg_n_per_l = concentrations.tan_mg_n_per_l();
    let nitrate_mg_n_per_l = concentrations.nitrate_mg_n_per_l();
    let phosphate_mg_p_per_l = concentrations.phosphate_mg_p_per_l();

    let previous_nuisance_index = state.algae.nuisance_index;

    // --- Light limitation ---
    // Combines photoperiod fraction and hardware intensity via half-saturation
    // kinetics.  Plant shading reduces available light proportional to average
    // crowding (up to 70% reduction, floored at 20% of full light).
    let light_factor = algae_light_factor(state);

    // --- Temperature limitation ---
    // Gaussian response: growth peaks at the optimum and decays symmetrically.
    let temp_factor = gaussian_response(
        state.water.temperature_c,
        state.process_params.algae_temp_optimum_c,
        state.process_params.algae_temp_sigma_c,
    );
    let plant_shading = (1.0
        - state
            .plant_guilds
            .iter()
            .map(|plant| plant.crowding_index)
            .sum::<f64>()
            / state.plant_guilds.len().max(1) as f64
            * 0.7)
        .clamp(0.2, 1.0);

    // --- Nutrient limitation (concentration-based Monod kinetics) ---
    // Dissolved inorganic nitrogen = TAN + NO₃⁻ (both in mg N / L).
    // Phosphorus = dissolved PO₄³⁻ (mg P / L).
    // Liebig's Law: the more limiting nutrient sets the factor.
    let nutrient_factor = half_saturation(
        tan_mg_n_per_l + nitrate_mg_n_per_l,
        state.process_params.algae_half_saturation_n_mg_n_per_l,
    )
    .min(half_saturation(
        phosphate_mg_p_per_l,
        state.process_params.algae_half_saturation_p_mg_p_per_l,
    ));
    let microfauna_grazing = (state.microfauna.population_index
        * (0.3 + 0.7 * state.microfauna.grazing_pressure_index))
        .clamp(0.0, 1.0);

    let suspended_seed_g = state.algae.suspended_biomass_g.max(0.02 * volume_l / 10.0);
    let suspended_gross_growth_g = suspended_seed_g
        * state.process_params.algae_max_growth_rate_per_day
        * light_factor
        * plant_shading
        * temp_factor
        * nutrient_factor;
    let suspended_respiration_g =
        state.algae.suspended_biomass_g * state.process_params.algae_respiration_fraction_per_day;
    let suspended_grazing_g = state.algae.suspended_biomass_g * 0.03 * microfauna_grazing;
    let (susp_nh3_removed, susp_no3_removed, susp_p_removed) = consume_algae_nutrients(
        state,
        suspended_gross_growth_g * ALGAE_N_MG_PER_G_BIOMASS,
        suspended_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH,
    );
    let susp_n_removed = susp_nh3_removed + susp_no3_removed;
    let susp_n_demand = suspended_gross_growth_g * ALGAE_N_MG_PER_G_BIOMASS;
    let susp_p_demand = suspended_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH;
    let susp_cap_frac = if suspended_gross_growth_g > f64::EPSILON {
        let n_frac = if susp_n_demand > f64::EPSILON {
            susp_n_removed / susp_n_demand
        } else {
            1.0
        };
        let p_frac = if susp_p_demand > f64::EPSILON {
            susp_p_removed / susp_p_demand
        } else {
            1.0
        };
        let c_demand = algae_carbon_mg(suspended_gross_growth_g, n_to_c_ratio);
        let c_frac = if c_demand > f64::EPSILON {
            state.water.dissolved_inorganic_carbon_mg_c_total / c_demand
        } else {
            1.0
        };
        n_frac.min(p_frac).min(c_frac)
    } else {
        0.0
    };
    let susp_cap = suspended_gross_growth_g * susp_cap_frac;
    // Refund only the non-limiting nutrient's excess.
    let susp_n_used = susp_n_demand * susp_cap_frac;
    let susp_p_used = susp_p_demand * susp_cap_frac;
    let susp_n_refund = (susp_n_removed - susp_n_used).max(0.0);
    let susp_p_refund = (susp_p_removed - susp_p_used).max(0.0);
    // Split N refund proportionally across ammonia/nitrate.
    let susp_n_frac = if susp_n_removed > f64::EPSILON {
        susp_n_refund / susp_n_removed
    } else {
        0.0
    };
    refund_algae_nutrients(
        state,
        susp_nh3_removed * susp_n_frac,
        susp_no3_removed * susp_n_frac,
        susp_p_refund,
    );
    if !hourly_photosynthesis_dic_enabled {
        // Closed-system tests zero the hourly shortcut and still need daily
        // biomass growth to withdraw carbon explicitly. When the hourly
        // chemistry path is enabled, it already owns the visible DIC/pH
        // response and a second midnight withdrawal creates false spikes.
        state.water.dissolved_inorganic_carbon_mg_c_total =
            (state.water.dissolved_inorganic_carbon_mg_c_total
                - algae_carbon_mg(susp_cap, n_to_c_ratio))
            .max(0.0);
    }
    let suspended_available_after_growth_g = (state.algae.suspended_biomass_g + susp_cap).max(0.0);
    let suspended_realized_loss_g = (suspended_respiration_g + suspended_grazing_g)
        .max(0.0)
        .min(suspended_available_after_growth_g);
    let suspended_new_g = (suspended_available_after_growth_g - suspended_realized_loss_g).max(0.0);
    state.algae.suspended_biomass_g = suspended_new_g;
    // Daily algae-loss fractions are modeled as particulate turnover here;
    // the separate hourly chemistry/DO passes own explicit respiration.
    route_algae_loss_to_fine_detritus(state, suspended_realized_loss_g, n_to_c_ratio);

    // TODO(tanksim-6e5.5.3): replace this legacy aggregate with habitat-based
    // periphyton capacity so configurable hardscape area contributes here too.
    let colonizable_area_m2 =
        total_colonizable_area_cm2(&state.substrate_layers, state.geometry.wall_area_cm2())
            / 10_000.0;
    let grazing_surface_factor =
        0.7 + (0.3 * state.avg_substrate_index(|layer| layer.grazing_surface_index));
    let periphyton_capacity_g = colonizable_area_m2.max(0.0)
        * state.process_params.periphyton_capacity_g_per_m2
        * grazing_surface_factor;
    let surface_cap_factor = if periphyton_capacity_g <= f64::EPSILON {
        0.0
    } else {
        (1.0 - (state.algae.periphyton_biomass_g / periphyton_capacity_g)).clamp(0.0, 1.0)
    };
    let periphyton_seed_g = state.algae.periphyton_biomass_g.max(0.05);
    let periphyton_gross_growth_g = periphyton_seed_g
        * state.process_params.periphyton_max_growth_rate_per_day
        * light_factor
        * plant_shading
        * temp_factor
        * nutrient_factor
        * surface_cap_factor;
    let periphyton_respiration_g =
        state.algae.periphyton_biomass_g * state.process_params.algae_respiration_fraction_per_day;
    let periphyton_grazing_g = state.algae.periphyton_biomass_g * 0.06 * microfauna_grazing;
    let (peri_nh3_removed, peri_no3_removed, peri_p_removed) = consume_algae_nutrients(
        state,
        periphyton_gross_growth_g * ALGAE_N_MG_PER_G_BIOMASS,
        periphyton_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH,
    );
    let peri_n_removed = peri_nh3_removed + peri_no3_removed;
    let peri_n_demand = periphyton_gross_growth_g * ALGAE_N_MG_PER_G_BIOMASS;
    let peri_p_demand = periphyton_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH;
    let peri_cap_frac = if periphyton_gross_growth_g > f64::EPSILON {
        let n_frac = if peri_n_demand > f64::EPSILON {
            peri_n_removed / peri_n_demand
        } else {
            1.0
        };
        let p_frac = if peri_p_demand > f64::EPSILON {
            peri_p_removed / peri_p_demand
        } else {
            1.0
        };
        let c_demand = algae_carbon_mg(periphyton_gross_growth_g, n_to_c_ratio);
        let c_frac = if c_demand > f64::EPSILON {
            state.water.dissolved_inorganic_carbon_mg_c_total / c_demand
        } else {
            1.0
        };
        n_frac.min(p_frac).min(c_frac)
    } else {
        0.0
    };
    let peri_cap = periphyton_gross_growth_g * peri_cap_frac;
    let peri_n_used = peri_n_demand * peri_cap_frac;
    let peri_p_used = peri_p_demand * peri_cap_frac;
    let peri_n_refund = (peri_n_removed - peri_n_used).max(0.0);
    let peri_p_refund = (peri_p_removed - peri_p_used).max(0.0);
    let peri_n_frac = if peri_n_removed > f64::EPSILON {
        peri_n_refund / peri_n_removed
    } else {
        0.0
    };
    refund_algae_nutrients(
        state,
        peri_nh3_removed * peri_n_frac,
        peri_no3_removed * peri_n_frac,
        peri_p_refund,
    );
    if !hourly_photosynthesis_dic_enabled {
        state.water.dissolved_inorganic_carbon_mg_c_total =
            (state.water.dissolved_inorganic_carbon_mg_c_total
                - algae_carbon_mg(peri_cap, n_to_c_ratio))
            .max(0.0);
    }
    let periphyton_available_after_growth_g =
        (state.algae.periphyton_biomass_g + peri_cap).max(0.0);
    let periphyton_realized_loss_g = (periphyton_respiration_g + periphyton_grazing_g)
        .max(0.0)
        .min(periphyton_available_after_growth_g);
    let periphyton_post_loss_g =
        (periphyton_available_after_growth_g - periphyton_realized_loss_g).max(0.0);
    let excess_g = (periphyton_post_loss_g - periphyton_capacity_g.max(0.0)).max(0.0);
    let periphyton_new_g = (periphyton_post_loss_g - excess_g).max(0.0);
    state.algae.periphyton_biomass_g = periphyton_new_g;
    route_algae_loss_to_fine_detritus(state, periphyton_realized_loss_g + excess_g, n_to_c_ratio);

    let suspended_pressure = (state.algae.suspended_biomass_g / volume_l)
        / state
            .process_params
            .algae_bloom_threshold_g_per_l
            .max(f64::MIN_POSITIVE);
    let periphyton_pressure = if periphyton_capacity_g <= f64::EPSILON {
        0.0
    } else {
        state.algae.periphyton_biomass_g / periphyton_capacity_g
    };
    state.algae.nuisance_index =
        (0.55 * suspended_pressure + 0.45 * periphyton_pressure).clamp(0.0, 1.0);

    events::emit_daily_algae_events(state, previous_nuisance_index, periphyton_capacity_g);
    resolve_carbonate_state(&mut state.water, volume_l);
}

/// Compute the effective light limitation factor for algae growth.
///
/// Combines three dimensionless drivers into a single [0, 1] factor:
/// 1. **Photoperiod**: hours of light per day normalised to a 9-hour
///    reference (longer days → more growth, capped at 1.0).
/// 2. **Intensity**: hardware light intensity index (0–1).
/// 3. **Depth/turbidity attenuation**: Beer-Lambert column-average factor
///    that reduces effective PAR in deeper or more turbid water.
///
/// The product of these three is the effective light dose; a
/// half-saturation curve then converts it to a limitation factor.
fn algae_light_factor(state: &TankState) -> f64 {
    if !state.hardware.light.enabled {
        return 0.0;
    }

    let photoperiod_factor = (state.hardware.light.photoperiod_hours / 9.0).clamp(0.0, 1.0);
    let k = state.extinction_coefficient();
    let h = state.water_depth_above_substrate_cm();
    let depth_factor = super::light::column_average_attenuation_factor(k, h);
    half_saturation(
        state.hardware.light.intensity_index * photoperiod_factor * depth_factor,
        state.process_params.algae_light_half_saturation,
    )
}

fn refund_algae_nutrients(
    state: &mut TankState,
    ammonia_refund_mg: f64,
    nitrate_refund_mg: f64,
    p_refund_mg: f64,
) {
    if ammonia_refund_mg > f64::EPSILON {
        state.water.ammonia_total_mg_n_total += ammonia_refund_mg;
    }
    if nitrate_refund_mg > f64::EPSILON {
        state.water.nitrate_mg_n_total += nitrate_refund_mg;
    }
    if p_refund_mg > f64::EPSILON {
        state.water.phosphate_mg_p_total += p_refund_mg;
    }
}

/// Returns (ammonia_removed, nitrate_removed, phosphate_removed) so
/// refunds can go back to the correct nitrogen pool.
fn consume_algae_nutrients(
    state: &mut TankState,
    n_demand_mg: f64,
    p_demand_mg: f64,
) -> (f64, f64, f64) {
    let ammonia_removed = state.water.ammonia_total_mg_n_total.min(n_demand_mg * 0.6);
    state.water.ammonia_total_mg_n_total -= ammonia_removed;

    let remaining_n = (n_demand_mg - ammonia_removed).max(0.0);
    let nitrate_removed = state.water.nitrate_mg_n_total.min(remaining_n);
    state.water.nitrate_mg_n_total -= nitrate_removed;

    let phosphate_removed = state.water.phosphate_mg_p_total.min(p_demand_mg);
    state.water.phosphate_mg_p_total -= phosphate_removed;

    (ammonia_removed, nitrate_removed, phosphate_removed)
}

/// C7 death/senescence interface for algae: respiration, microfauna
/// grazing, and excess shedding all enter fine_detritus_g_total,
/// joining feed-derived waste in the same downstream dissolution ->
/// mineralization path.  See docs/MASS_FLOW.md, section 9.
fn route_algae_loss_to_fine_detritus(state: &mut TankState, biomass_g: f64, n_to_c_ratio: f64) {
    state.detritus.fine_detritus_g_total += algae_detrital_mass_g(biomass_g, n_to_c_ratio);
}

fn half_saturation(value: f64, half_sat: f64) -> f64 {
    let half_sat = half_sat.max(f64::MIN_POSITIVE);
    (value / (value + half_sat)).clamp(0.0, 1.0)
}

fn gaussian_response(value: f64, optimum: f64, sigma: f64) -> f64 {
    let sigma = sigma.max(0.1);
    let diff = value - optimum;
    (-(diff * diff) / (2.0 * sigma * sigma))
        .exp()
        .clamp(0.0, 1.0)
}
