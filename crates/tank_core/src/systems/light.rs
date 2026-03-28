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
    use super::is_light_on;

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
}
