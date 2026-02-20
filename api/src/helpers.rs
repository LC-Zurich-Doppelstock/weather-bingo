//! Shared helpers for Decimal ↔ f64 conversions.
//!
//! Two f64→Decimal strategies exist because weather values and geo coordinates
//! have different precision requirements:
//!
//! - `f64_to_decimal_1dp`: rounds to 1 decimal place (weather: temperature, wind, etc.)
//! - `f64_to_decimal_full`: preserves full f64 precision (geo: lat, lon, elevation, distance)
//!
//! Both return `Decimal::ZERO` for non-finite inputs (NaN, ±Inf).

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

/// Convert an f64 to Decimal, rounded to 1 decimal place.
///
/// Used for weather values (temperature, wind speed, etc.) where 0.1°C / 0.1 m/s
/// precision is sufficient and consistent rounding avoids false uniqueness in dedup.
pub(crate) fn f64_to_decimal_1dp(v: f64) -> Decimal {
    if !v.is_finite() {
        tracing::warn!(
            "f64_to_decimal_1dp received non-finite value {}, defaulting to 0",
            v
        );
        return Decimal::ZERO;
    }
    Decimal::from_str_exact(&format!("{:.1}", v)).unwrap_or_default()
}

/// Convert an optional f64 to Decimal (1 decimal place), returning None if input is None.
pub(crate) fn opt_f64_to_decimal_1dp(v: Option<f64>) -> Option<Decimal> {
    v.map(f64_to_decimal_1dp)
}

/// Convert an f64 to Decimal preserving full precision.
///
/// Used for geographic values (latitude, longitude, elevation, distance) where
/// full precision matters for accurate positioning.
pub(crate) fn f64_to_decimal_full(v: f64) -> Decimal {
    if !v.is_finite() {
        tracing::warn!(
            "f64_to_decimal_full received non-finite value {}, defaulting to 0",
            v
        );
        return Decimal::ZERO;
    }
    Decimal::from_f64(v).unwrap_or_else(|| Decimal::new(v as i64, 0))
}

/// Convert a Decimal to f64, defaulting to 0.0 for values that can't be represented.
///
/// Replaces the repeated pattern `some_decimal.to_f64().unwrap_or(0.0)`.
pub(crate) fn dec_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

/// Convert an Option<Decimal> to Option<f64>.
pub(crate) fn opt_dec_to_f64(d: Option<Decimal>) -> Option<f64> {
    d.and_then(|v| v.to_f64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_f64_to_decimal_1dp_normal() {
        let d = f64_to_decimal_1dp(3.14);
        assert_eq!(d, Decimal::from_str("3.1").unwrap());
    }

    #[test]
    fn test_f64_to_decimal_1dp_rounds() {
        // 3.16 rounded to 1dp → 3.2
        let d = f64_to_decimal_1dp(3.16);
        assert_eq!(d, Decimal::from_str("3.2").unwrap());
    }

    #[test]
    fn test_f64_to_decimal_1dp_nan() {
        assert_eq!(f64_to_decimal_1dp(f64::NAN), Decimal::ZERO);
    }

    #[test]
    fn test_f64_to_decimal_1dp_infinity() {
        assert_eq!(f64_to_decimal_1dp(f64::INFINITY), Decimal::ZERO);
    }

    #[test]
    fn test_f64_to_decimal_full_normal() {
        let d = f64_to_decimal_full(3.14);
        assert!(d > Decimal::ZERO);
    }

    #[test]
    fn test_f64_to_decimal_full_nan() {
        assert_eq!(f64_to_decimal_full(f64::NAN), Decimal::ZERO);
    }

    #[test]
    fn test_f64_to_decimal_full_infinity() {
        assert_eq!(f64_to_decimal_full(f64::INFINITY), Decimal::ZERO);
    }

    #[test]
    fn test_f64_to_decimal_full_neg_infinity() {
        assert_eq!(f64_to_decimal_full(f64::NEG_INFINITY), Decimal::ZERO);
    }

    #[test]
    fn test_dec_to_f64_normal() {
        let d = Decimal::from_str("3.14").unwrap();
        assert!((dec_to_f64(d) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_dec_to_f64_zero() {
        assert_eq!(dec_to_f64(Decimal::ZERO), 0.0);
    }

    #[test]
    fn test_opt_f64_to_decimal_1dp() {
        assert_eq!(opt_f64_to_decimal_1dp(None), None);
        assert_eq!(
            opt_f64_to_decimal_1dp(Some(3.14)),
            Some(Decimal::from_str("3.1").unwrap())
        );
    }

    #[test]
    fn test_opt_dec_to_f64() {
        assert_eq!(opt_dec_to_f64(None), None);
        let d = Decimal::from_str("3.14").unwrap();
        assert!((opt_dec_to_f64(Some(d)).unwrap() - 3.14).abs() < 1e-10);
    }
}
