//! Element-to-ion conversion factors for aquarium chemistry.
//!
//! All snapshot fields store concentrations on an element basis (mg N/L, mg P/L,
//! mg C/L).  These constants convert to/from the ion-mass basis that hobbyist
//! test kits and some references use (mg NO3/L, mg PO4/L, etc.).
//!
//! Each factor equals (molar mass of ion) / (molar mass of element).
//! See docs/UNITS.md for the canonical field-naming policy.

// ---------- forward: element → ion ----------

/// NO₃⁻ / N  =  62.004 / 14.007 ≈ 4.427
pub const NO3_PER_N: f64 = 62.004 / 14.007;

/// NO₂⁻ / N  =  46.005 / 14.007 ≈ 3.284
pub const NO2_PER_N: f64 = 46.005 / 14.007;

/// NH₄⁺ / N  =  18.039 / 14.007 ≈ 1.288
pub const NH4_PER_N: f64 = 18.039 / 14.007;

/// NH₃  / N  =  17.031 / 14.007 ≈ 1.216
pub const NH3_PER_N: f64 = 17.031 / 14.007;

/// PO₄³⁻ / P  =  94.971 / 30.974 ≈ 3.066
pub const PO4_PER_P: f64 = 94.971 / 30.974;

// ---------- forward helpers ----------

/// Convert nitrate from mg N/L → mg NO₃/L.
pub fn nitrate_mg_no3_per_l(n_per_l: f64) -> f64 {
    n_per_l * NO3_PER_N
}

/// Convert nitrite from mg N/L → mg NO₂/L.
pub fn nitrite_mg_no2_per_l(n_per_l: f64) -> f64 {
    n_per_l * NO2_PER_N
}

/// Convert ammonium from mg N/L → mg NH₄/L.
pub fn ammonium_mg_nh4_per_l(n_per_l: f64) -> f64 {
    n_per_l * NH4_PER_N
}

/// Convert ammonia from mg N/L → mg NH₃/L.
pub fn ammonia_mg_nh3_per_l(n_per_l: f64) -> f64 {
    n_per_l * NH3_PER_N
}

/// Convert phosphate from mg P/L → mg PO₄/L.
pub fn phosphate_mg_po4_per_l(p_per_l: f64) -> f64 {
    p_per_l * PO4_PER_P
}

// ---------- inverse helpers ----------

/// Convert nitrate from mg NO₃/L → mg N/L.
pub fn nitrate_n_from_no3(no3_per_l: f64) -> f64 {
    no3_per_l / NO3_PER_N
}

/// Convert nitrite from mg NO₂/L → mg N/L.
pub fn nitrite_n_from_no2(no2_per_l: f64) -> f64 {
    no2_per_l / NO2_PER_N
}

/// Convert ammonium from mg NH₄/L → mg N/L.
pub fn ammonium_n_from_nh4(nh4_per_l: f64) -> f64 {
    nh4_per_l / NH4_PER_N
}

/// Convert ammonia from mg NH₃/L → mg N/L.
pub fn ammonia_n_from_nh3(nh3_per_l: f64) -> f64 {
    nh3_per_l / NH3_PER_N
}

/// Convert phosphate from mg PO₄/L → mg P/L.
pub fn phosphate_p_from_po4(po4_per_l: f64) -> f64 {
    po4_per_l / PO4_PER_P
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() < tol,
            "expected {a} ≈ {b} (tol {tol}), delta = {}",
            (a - b).abs()
        );
    }

    #[test]
    fn reference_values() {
        assert_close(NO3_PER_N, 4.427, 0.001);
        assert_close(NO2_PER_N, 3.284, 0.001);
        assert_close(NH4_PER_N, 1.288, 0.001);
        assert_close(NH3_PER_N, 1.216, 0.001);
        assert_close(PO4_PER_P, 3.066, 0.001);
    }

    #[test]
    fn round_trip_nitrate() {
        let n = 10.0;
        let ion = nitrate_mg_no3_per_l(n);
        let back = nitrate_n_from_no3(ion);
        assert_close(back, n, 1e-12);
    }

    #[test]
    fn round_trip_nitrite() {
        let n = 0.5;
        let ion = nitrite_mg_no2_per_l(n);
        let back = nitrite_n_from_no2(ion);
        assert_close(back, n, 1e-12);
    }

    #[test]
    fn round_trip_ammonium() {
        let n = 2.0;
        let ion = ammonium_mg_nh4_per_l(n);
        let back = ammonium_n_from_nh4(ion);
        assert_close(back, n, 1e-12);
    }

    #[test]
    fn round_trip_ammonia() {
        let n = 0.02;
        let ion = ammonia_mg_nh3_per_l(n);
        let back = ammonia_n_from_nh3(ion);
        assert_close(back, n, 1e-12);
    }

    #[test]
    fn round_trip_phosphate() {
        let p = 1.0;
        let ion = phosphate_mg_po4_per_l(p);
        let back = phosphate_p_from_po4(ion);
        assert_close(back, p, 1e-12);
    }

    #[test]
    fn zero_stays_zero() {
        assert_eq!(nitrate_mg_no3_per_l(0.0), 0.0);
        assert_eq!(nitrite_mg_no2_per_l(0.0), 0.0);
        assert_eq!(ammonium_mg_nh4_per_l(0.0), 0.0);
        assert_eq!(ammonia_mg_nh3_per_l(0.0), 0.0);
        assert_eq!(phosphate_mg_po4_per_l(0.0), 0.0);
    }
}
