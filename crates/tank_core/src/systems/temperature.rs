use crate::types::TankState;

/// DO saturation in mg/L from linear interpolation over the reference table:
/// 0°C → 14.6, 10°C → 11.3, 20°C → 9.1, 30°C → 7.6
pub fn do_sat_mg_l(temp_c: f64) -> f64 {
    const TABLE: [(f64, f64); 5] = [
        (0.0, 14.6),
        (10.0, 11.3),
        (20.0, 9.1),
        (30.0, 7.6),
        (40.0, 6.4),
    ];

    if temp_c <= TABLE[0].0 {
        return TABLE[0].1;
    }
    if temp_c >= TABLE[TABLE.len() - 1].0 {
        return TABLE[TABLE.len() - 1].1;
    }

    for i in 0..TABLE.len() - 1 {
        let (t0, do0) = TABLE[i];
        let (t1, do1) = TABLE[i + 1];
        if temp_c >= t0 && temp_c <= t1 {
            let frac = (temp_c - t0) / (t1 - t0);
            return do0 + frac * (do1 - do0);
        }
    }

    // Fallback (shouldn't reach here)
    TABLE[TABLE.len() - 1].1
}

/// Hourly discrete heat-transfer model.
///
/// Updates `state.water.temperature_c` based on ambient temperature, heater,
/// and tank geometry. Records heater output on `state.hardware.heater.last_output_w`.
pub fn step_temperature(state: &mut TankState) {
    let dt_s = 3600.0_f64;
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let heat_capacity_j_per_k = volume_l * 4186.0;

    let surface_area_m2 = state.geometry.surface_area_cm2() / 10_000.0;
    let wall_area_m2 = state.geometry.wall_area_cm2() / 10_000.0;
    let top_factor = state.geometry.top_exchange_factor();

    let k_surface = state.process_params.k_surface_w_per_m2_k;
    let k_wall = state.process_params.k_wall_w_per_m2_k;

    let ua_total_w_per_k = (k_surface * surface_area_m2 * top_factor) + (k_wall * wall_area_m2);

    let ambient_temp_c = state.environment.ambient_temp_c;
    let water_temp_c = state.water.temperature_c;

    let q_ambient_w = ua_total_w_per_k * (ambient_temp_c - water_temp_c);

    // Heater: only fires when enabled and water is below setpoint - deadband/2
    let q_heater_w = if state.hardware.heater.enabled
        && water_temp_c
            < (state.hardware.heater.setpoint_c - state.hardware.heater.deadband_c / 2.0)
    {
        (state.hardware.heater.max_watts * state.hardware.heater.efficiency).max(0.0)
    } else {
        0.0
    };

    // Apply ambient heat exchange first, then add heater energy capped so it
    // cannot push the temperature above the setpoint.  This way a hot room can
    // still warm the tank beyond the setpoint (the heater has no cooling path)
    // while the thermostat duty-cycle stops heating once the target is reached.
    let delta_ambient_c = q_ambient_w * dt_s / heat_capacity_j_per_k;
    state.water.temperature_c += delta_ambient_c;

    if q_heater_w > 0.0 {
        let delta_heater_c = q_heater_w * dt_s / heat_capacity_j_per_k;
        let headroom = (state.hardware.heater.setpoint_c - state.water.temperature_c).max(0.0);
        let realized_heater_c = delta_heater_c.min(headroom);
        state.water.temperature_c += realized_heater_c;
        // Record the actually-used heater power, not the commanded watts.
        let realized_fraction = if delta_heater_c > f64::EPSILON {
            realized_heater_c / delta_heater_c
        } else {
            0.0
        };
        state.hardware.heater.last_output_w = q_heater_w * realized_fraction;
    } else {
        state.hardware.heater.last_output_w = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn do_sat_boundaries() {
        assert!((do_sat_mg_l(0.0) - 14.6).abs() < 0.01);
        assert!((do_sat_mg_l(10.0) - 11.3).abs() < 0.01);
        assert!((do_sat_mg_l(20.0) - 9.1).abs() < 0.01);
        assert!((do_sat_mg_l(30.0) - 7.6).abs() < 0.01);
    }

    #[test]
    fn do_sat_interpolation() {
        let mid = do_sat_mg_l(15.0);
        // Midpoint between 11.3 and 9.1 = 10.2
        assert!((mid - 10.2).abs() < 0.01);
    }

    #[test]
    fn do_sat_clamp() {
        assert!((do_sat_mg_l(-5.0) - 14.6).abs() < 0.01);
        assert!((do_sat_mg_l(50.0) - 6.4).abs() < 0.01);
    }

    #[test]
    fn do_sat_above_30() {
        let at_35 = do_sat_mg_l(35.0);
        // Midpoint between 7.6 (30°C) and 6.4 (40°C) = 7.0
        assert!((at_35 - 7.0).abs() < 0.01);
        assert!(do_sat_mg_l(30.0) > do_sat_mg_l(35.0));
    }
}
