/// Beer-Lambert attenuation factor at depth z (cm) given extinction
/// coefficient k (1/cm).
///
/// Returns exp(−k × z), the fraction of surface light intensity
/// remaining at depth z.
pub fn beer_lambert_at_depth(k: f64, z: f64) -> f64 {
    (-k * z).exp()
}

/// Column-average attenuation factor for a water column of height h (cm)
/// with extinction coefficient k (1/cm).
///
/// Returns (1 − exp(−k×H)) / (k×H), the mean fraction of surface light
/// across the entire water column.  For k×H → 0 (clear/shallow),
/// returns 1.0.
pub fn column_average_attenuation_factor(k: f64, h: f64) -> f64 {
    let kh = k * h;
    if kh < 1e-9 {
        return 1.0;
    }
    ((1.0 - (-kh).exp()) / kh).clamp(0.0, 1.0)
}

pub fn is_light_on(hour_of_day: u8, photoperiod_hours: f64) -> bool {
    if photoperiod_hours <= 0.0 {
        return false;
    }
    if photoperiod_hours >= 24.0 {
        return true;
    }

    let midpoint_hour = f64::from(hour_of_day % 24) + 0.5;
    let start_hour = 12.0 - photoperiod_hours / 2.0;
    let end_hour = 12.0 + photoperiod_hours / 2.0;

    midpoint_hour >= start_hour && midpoint_hour < end_hour
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_on_noon_for_even_photoperiod() {
        let lit_hours: Vec<u8> = (0..24).filter(|hour| is_light_on(*hour, 8.0)).collect();
        assert_eq!(lit_hours, vec![8, 9, 10, 11, 12, 13, 14, 15]);
    }

    #[test]
    fn zero_and_full_day_photoperiods() {
        assert!(!(0..24).any(|hour| is_light_on(hour, 0.0)));
        assert!((0..24).all(|hour| is_light_on(hour, 24.0)));
    }

    // --- Beer-Lambert unit tests (acceptance criteria) ---

    /// AC: Beer-Lambert at k=0.2, z=20cm → ≈0.018 (within 0.001).
    #[test]
    fn beer_lambert_k02_z20_matches_expected() {
        let result = beer_lambert_at_depth(0.2, 20.0);
        let expected = (-0.2_f64 * 20.0).exp(); // e^-4 ≈ 0.01832
        assert!(
            (result - expected).abs() < 1e-6,
            "exact formula check: got {result}, expected {expected}"
        );
        assert!(
            (result - 0.018).abs() < 0.001,
            "AC tolerance: got {result}, expected ~0.018"
        );
    }

    /// AC: Column-average light matches I_avg = I_0 × (1 − exp(−k×H)) / (k×H).
    #[test]
    fn column_average_matches_analytical_formula() {
        let k = 0.15_f64;
        let h = 25.0_f64;
        let kh = k * h;
        let expected = (1.0 - (-kh).exp()) / kh;
        let result = column_average_attenuation_factor(k, h);
        assert!(
            (result - expected).abs() < 1e-9,
            "got {result}, expected {expected}"
        );
    }

    /// Attenuation increases monotonically with depth.
    #[test]
    fn beer_lambert_decreases_with_depth() {
        let k = 0.05;
        let depths = [0.0, 5.0, 10.0, 20.0, 40.0];
        for window in depths.windows(2) {
            let shallow = beer_lambert_at_depth(k, window[0]);
            let deep = beer_lambert_at_depth(k, window[1]);
            assert!(
                shallow > deep,
                "light at z={} ({shallow}) should exceed light at z={} ({deep})",
                window[0],
                window[1]
            );
        }
    }

    /// Higher turbidity (larger k) reduces light at a fixed depth.
    #[test]
    fn beer_lambert_decreases_with_turbidity() {
        let z = 15.0;
        let ks = [0.01, 0.05, 0.1, 0.3];
        for window in ks.windows(2) {
            let clear = beer_lambert_at_depth(window[0], z);
            let turbid = beer_lambert_at_depth(window[1], z);
            assert!(
                clear > turbid,
                "k={} ({clear}) should transmit more than k={} ({turbid})",
                window[0],
                window[1]
            );
        }
    }

    /// Zero turbidity (pure water, k=0) transmits all light.
    #[test]
    fn zero_extinction_transmits_all_light() {
        assert!((beer_lambert_at_depth(0.0, 50.0) - 1.0).abs() < 1e-12);
        assert!((column_average_attenuation_factor(0.0, 50.0) - 1.0).abs() < 1e-12);
    }

    /// Column average is always between surface (1.0) and bottom for k > 0.
    #[test]
    fn column_average_bounded_between_surface_and_bottom() {
        let k = 0.1;
        let h = 30.0;
        let avg = column_average_attenuation_factor(k, h);
        let bottom = beer_lambert_at_depth(k, h);
        assert!(avg < 1.0, "avg {avg} should be less than surface (1.0)");
        assert!(
            avg > bottom,
            "avg {avg} should exceed bottom intensity {bottom}"
        );
    }

    /// Shallow water with low k should have column average close to 1.0.
    #[test]
    fn shallow_clear_water_column_average_near_unity() {
        let avg = column_average_attenuation_factor(0.001, 5.0);
        assert!(
            avg > 0.99,
            "5 cm of very clear water should transmit >99%: got {avg}"
        );
    }
}
