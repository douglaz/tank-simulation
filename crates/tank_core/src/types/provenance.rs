//! Provenance metadata lives alongside preset-layer values, not inside the
//! runtime state structs in `tank_core`.
//!
//! `tank_data` preset types can attach `ParamMeta` to TOML-backed parameters,
//! including values that fall back to deterministic code defaults during preset
//! materialization. Calling `Default::default()` on runtime structs such as
//! `ProcessParams` or `ShrimpRuntimeParams` yields numeric defaults only; those
//! code-defined values do not carry inline provenance unless a preset or report
//! stores matching `ParamMeta` next to them.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Confidence level for a scientific parameter value.
///
/// Ordered from highest to lowest confidence. Unknown values produce a
/// deserialization error so that typos are caught early.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    /// Value sourced from peer-reviewed literature or authoritative reference.
    Literature,
    /// Value recommended by a domain expert without a specific citation.
    Expert,
    /// Value derived from rules of thumb, analogy, or calibration.
    Heuristic,
    /// Temporary stand-in value awaiting better data.
    Placeholder,
}

impl fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literature => f.write_str("literature"),
            Self::Expert => f.write_str("expert"),
            Self::Heuristic => f.write_str("heuristic"),
            Self::Placeholder => f.write_str("placeholder"),
        }
    }
}

/// Per-parameter provenance metadata.
///
/// All fields are optional so that provenance can be attached incrementally.
/// A minimal entry might carry only `unit`; a fully-documented entry carries
/// everything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamMeta {
    /// Physical unit string, e.g. `"mg N/L"`, `"per hour"`, `"W/(m²·K)"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable citation or source description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// How much we trust this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ConfidenceLevel>,

    /// Inclusive `[low, high]` range within which the value is expected to fall
    /// under normal conditions. Values outside this range produce a warning,
    /// not an error, because heuristic tuning may intentionally push values
    /// beyond literature bounds. For legacy process fields whose names still
    /// end in `_mg` or `_mg_total`, express `valid_range` in the normalized
    /// concentration units shown by `unit` (for example `mg N/L`), not in the
    /// raw historical storage totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_range: Option<[f64; 2]>,

    /// Free-form notes for developers and reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl fmt::Display for ParamMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format: unit [confidence, source]
        // Example: "mg N/L [literature, EPA 2013 ammonia criteria]"
        let mut parts = Vec::new();
        if let Some(ref unit) = self.unit {
            parts.push(unit.clone());
        }
        let mut bracket_parts = Vec::new();
        if let Some(confidence) = self.confidence {
            bracket_parts.push(confidence.to_string());
        }
        if let Some(ref source) = self.source {
            bracket_parts.push(source.clone());
        }
        if !bracket_parts.is_empty() {
            parts.push(format!("[{}]", bracket_parts.join(", ")));
        }
        if let Some([lo, hi]) = self.valid_range {
            // Use Debug formatting for floats to ensure trailing `.0`.
            parts.push(format!("range [{lo:?}, {hi:?}]"));
        }
        if let Some(ref notes) = self.notes {
            parts.push(format!("notes: {notes}"));
        }
        write!(f, "{}", parts.join(" "))
    }
}

/// A warning emitted when a parameter value falls outside its declared
/// `valid_range`. This is informational, not a load failure.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeWarning {
    pub param_name: String,
    pub value: f64,
    pub range: [f64; 2],
}

impl fmt::Display for RangeWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.range[0].is_finite() || !self.range[1].is_finite() || self.range[0] > self.range[1]
        {
            return write!(
                f,
                "parameter `{}` declares invalid valid_range [{}, {}]",
                self.param_name, self.range[0], self.range[1]
            );
        }
        if !self.value.is_finite() {
            return write!(
                f,
                "parameter `{}` value {} is non-finite and cannot be checked against valid range [{}, {}]",
                self.param_name, self.value, self.range[0], self.range[1]
            );
        }
        write!(
            f,
            "parameter `{}` value {} is outside valid range [{}, {}]",
            self.param_name, self.value, self.range[0], self.range[1]
        )
    }
}

/// Check a single parameter value against its metadata's valid_range.
/// Returns `Some(RangeWarning)` if the value is outside the range.
pub fn check_param_range(name: &str, value: f64, meta: &ParamMeta) -> Option<RangeWarning> {
    if let Some([lo, hi]) = meta.valid_range {
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Some(RangeWarning {
                param_name: name.to_string(),
                value,
                range: [lo, hi],
            });
        }
        if !value.is_finite() {
            return Some(RangeWarning {
                param_name: name.to_string(),
                value,
                range: [lo, hi],
            });
        }
        if value < lo || value > hi {
            return Some(RangeWarning {
                param_name: name.to_string(),
                value,
                range: [lo, hi],
            });
        }
    }
    None
}

/// Check all parameters in a metadata map against their valid ranges.
///
/// `values` is a lookup function that returns the current value for a
/// parameter name, or `None` if the parameter is not present.
pub fn check_all_ranges(
    param_meta: &BTreeMap<String, ParamMeta>,
    values: &dyn Fn(&str) -> Option<f64>,
) -> Vec<RangeWarning> {
    let mut warnings = Vec::new();
    for (name, meta) in param_meta {
        if let Some(value) = values(name) {
            if let Some(w) = check_param_range(name, value, meta) {
                warnings.push(w);
            }
        }
    }
    warnings
}

/// Format a named parameter with its value and provenance metadata for
/// developer display (docs, debug output, reports).
///
/// Example output: `"aob_k_tan_mg: 0.5 mg N/L [literature, EPA 2013]"`
pub fn format_param(name: &str, value: f64, meta: &ParamMeta) -> String {
    let meta_str = meta.to_string();
    if meta_str.is_empty() {
        format!("{name}: {value}")
    } else {
        format!("{name}: {value} {meta_str}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_level_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        for level in [
            ConfidenceLevel::Literature,
            ConfidenceLevel::Expert,
            ConfidenceLevel::Heuristic,
            ConfidenceLevel::Placeholder,
        ] {
            let json = serde_json::to_string(&level)?;
            let back: ConfidenceLevel = serde_json::from_str(&json)?;
            assert_eq!(level, back);
        }
        Ok(())
    }

    #[test]
    fn test_confidence_level_rejects_unknown() {
        let result: Result<ConfidenceLevel, _> = serde_json::from_str("\"medium\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_param_meta_display_full() {
        let meta = ParamMeta {
            unit: Some("mg N/L".into()),
            source: Some("EPA 2013".into()),
            confidence: Some(ConfidenceLevel::Literature),
            valid_range: Some([0.1, 5.0]),
            notes: Some("test".into()),
        };
        let display = meta.to_string();
        assert!(display.contains("mg N/L"));
        assert!(display.contains("literature"));
        assert!(display.contains("EPA 2013"));
        assert!(display.contains("range [0.1, 5.0]"), "got: {display}");
        assert!(display.contains("notes: test"), "got: {display}");
    }

    #[test]
    fn test_param_meta_display_empty() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: None,
            notes: None,
        };
        assert_eq!(meta.to_string(), "");
    }

    #[test]
    fn test_range_check_in_range() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([0.1, 5.0]),
            notes: None,
        };
        assert!(check_param_range("k", 0.5, &meta).is_none());
    }

    #[test]
    fn test_range_check_out_of_range() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([0.1, 5.0]),
            notes: None,
        };
        let w = check_param_range("k", 10.0, &meta).expect("should warn");
        assert_eq!(w.param_name, "k");
        assert_eq!(w.value, 10.0);
    }

    #[test]
    fn test_range_check_non_finite_value_warns() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([0.1, 5.0]),
            notes: None,
        };
        let w = check_param_range("k", f64::NAN, &meta).expect("should warn");
        assert!(w.value.is_nan());
        assert_eq!(w.range, [0.1, 5.0]);
        assert!(w.to_string().contains("non-finite"));
    }

    #[test]
    fn test_range_check_invalid_range_warns() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([5.0, 0.1]),
            notes: None,
        };
        let w = check_param_range("k", 1.0, &meta).expect("should warn");
        assert_eq!(w.range, [5.0, 0.1]);
        assert!(w.to_string().contains("invalid valid_range"));
    }

    #[test]
    fn test_range_check_no_range() {
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: None,
            notes: None,
        };
        assert!(check_param_range("k", 999.0, &meta).is_none());
    }

    #[test]
    fn test_format_param() {
        let meta = ParamMeta {
            unit: Some("mg N/L".into()),
            source: Some("EPA 2013".into()),
            confidence: Some(ConfidenceLevel::Literature),
            valid_range: None,
            notes: None,
        };
        let s = format_param("aob_k_tan_mg", 0.5, &meta);
        assert_eq!(s, "aob_k_tan_mg: 0.5 mg N/L [literature, EPA 2013]");
    }

    #[test]
    fn test_format_param_includes_notes() {
        let meta = ParamMeta {
            unit: Some("mg N/L".into()),
            source: Some("EPA 2013".into()),
            confidence: Some(ConfidenceLevel::Literature),
            valid_range: Some([0.1, 5.0]),
            notes: Some("biofilter context".into()),
        };
        let s = format_param("aob_k_tan_mg", 0.5, &meta);
        assert_eq!(
            s,
            "aob_k_tan_mg: 0.5 mg N/L [literature, EPA 2013] range [0.1, 5.0] notes: biofilter context"
        );
    }
}
