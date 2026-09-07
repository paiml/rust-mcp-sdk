//! Deterministic, decimal-aware Excel rounding helpers (finding #10).
//!
//! Naive `(x * 10f64.powi(d)).round() / 10f64.powi(d)` is NOT enough: the half
//! case (`1594.925`) is stored in binary `f64` as a value slightly BELOW
//! `1594.925`, so a naive scale-then-`round` yields `1594.92`, not Excel's
//! `1594.93`. These helpers apply a small epsilon correction at the rounding
//! boundary so the documented Excel half-away-from-zero behaviour is stable, and
//! they implement Excel's away-from-zero rules for ROUNDUP and CEILING so
//! NEGATIVE inputs are correct.
//!
//! - [`excel_round`] — round half away from zero to `digits` decimals.
//! - [`excel_roundup`] — round AWAY FROM ZERO to `digits` decimals (Excel ROUNDUP).
//! - [`excel_rounddown`] — round TOWARD ZERO to `digits` decimals (Excel ROUNDDOWN).
//! - [`excel_ceiling`] — round AWAY FROM ZERO to the nearest multiple of
//!   `significance` (Excel CEILING, magnitude rule).

/// The relative epsilon applied at the rounding boundary to undo binary-`f64`
/// representation error for the documented decimal half cases (e.g. `1594.925`
/// stored just under its decimal value). Scaled by the magnitude of the value.
const ROUND_EPSILON: f64 = 1e-9;

/// `10^digits` as `f64`, supporting negative `digits` (round to tens/hundreds).
fn pow10(digits: i32) -> f64 {
    10f64.powi(digits)
}

/// Excel `ROUND(x, digits)` — round half AWAY FROM ZERO to `digits` decimals.
///
/// A non-finite input passes through unchanged (the caller maps non-finite to an
/// Excel error above this layer).
pub fn excel_round(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let factor = pow10(digits);
    let scaled = x * factor;
    // Nudge by a magnitude-scaled epsilon toward away-from-zero so a decimal
    // half that binary-f64 stores just under its true value still rounds up.
    let nudged = scaled + scaled.signum() * (scaled.abs() * ROUND_EPSILON);
    // round() in Rust is already half-away-from-zero.
    nudged.round() / factor
}

/// Excel `ROUNDUP(x, digits)` — round AWAY FROM ZERO to `digits` decimals.
///
/// `ROUNDUP(3.001, 2) == 3.01`; `ROUNDUP(-3.001, 2) == -3.01` (magnitude grows,
/// sign preserved). A non-finite input passes through unchanged.
pub fn excel_roundup(x: f64, digits: i32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let factor = pow10(digits);
    let scaled = x * factor;
    // Away-from-zero: ceil the magnitude. Apply an epsilon PULL toward zero so a
    // value that is exactly representable (or a hair over due to f64 error) is
    // not spuriously bumped to the next integer.
    let pulled = scaled - scaled.signum() * (scaled.abs() * ROUND_EPSILON);
    let away = if pulled >= 0.0 {
        pulled.ceil()
    } else {
        pulled.floor()
    };
    away / factor
}

/// Excel `ROUNDDOWN(x, digits)` — round TOWARD ZERO to `digits` decimals.
///
/// The structural mirror of [`excel_roundup`]: `ROUNDDOWN(3.999, 2) == 3.99`;
/// `ROUNDDOWN(-3.999, 2) == -3.99` (magnitude SHRINKS, sign preserved). A
/// non-finite input passes through unchanged.
pub fn excel_rounddown(x: f64, digits: i32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let factor = pow10(digits);
    let scaled = x * factor;
    // Toward-zero: truncate the magnitude. Apply an epsilon PUSH away from zero
    // — the inverse of `excel_roundup`'s pull — so a value that is exactly
    // representable (or a hair under its decimal value due to f64 error) is not
    // spuriously dropped a whole step.
    let pushed = scaled + scaled.signum() * (scaled.abs() * ROUND_EPSILON);
    pushed.trunc() / factor
}

/// Excel `CEILING(number, significance)` — round `number` AWAY FROM ZERO to the
/// nearest multiple of `significance` (Excel's magnitude rule).
///
/// - `CEILING(10, 3) == 12`.
/// - `CEILING(-10, -3) == -12` (negative number, negative significance →
///   away-from-zero magnitude).
/// - `significance == 0` → `0` (Excel returns 0 for a zero significance).
/// - A `number`/`significance` sign mismatch returns `NaN` (Excel's `#NUM!`
///   case — the caller maps non-finite to an Excel error above this layer).
pub fn excel_ceiling(number: f64, significance: f64) -> f64 {
    if !number.is_finite() || !significance.is_finite() {
        return f64::NAN;
    }
    if significance == 0.0 {
        return 0.0;
    }
    // Excel: a positive number with a negative significance (or vice versa) is a
    // #NUM! error — signal it as NaN for the caller to map.
    if number != 0.0 && number.signum() != significance.signum() {
        return f64::NAN;
    }
    let ratio = number / significance;
    // Away-from-zero on the multiple count, with a small epsilon pull so an
    // exact multiple is not bumped to the next one by f64 error.
    let pulled = ratio - ratio.signum() * (ratio.abs() * ROUND_EPSILON);
    let steps = if pulled >= 0.0 {
        pulled.ceil()
    } else {
        pulled.floor()
    };
    steps * significance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_half_away_from_zero_decimal_boundary() {
        // The lighthouse golden boundary: 1594.925 → 1594.93 (NOT 1594.92).
        assert_eq!(excel_round(1594.925, 2), 1594.93);
        assert_eq!(excel_round(2.5, 0), 3.0);
        assert_eq!(excel_round(-2.5, 0), -3.0); // half away from zero
        assert_eq!(excel_round(2.4, 0), 2.0);
    }

    #[test]
    fn round_to_negative_digits() {
        assert_eq!(excel_round(1234.0, -2), 1200.0);
        assert_eq!(excel_round(1250.0, -2), 1300.0);
    }

    #[test]
    fn roundup_is_away_from_zero() {
        assert_eq!(excel_roundup(3.001, 2), 3.01);
        assert_eq!(excel_roundup(-3.001, 2), -3.01);
        assert_eq!(excel_roundup(3.0, 2), 3.0); // exact multiple not bumped
        assert_eq!(excel_roundup(0.0, 2), 0.0);
    }

    #[test]
    fn rounddown_is_toward_zero() {
        // The exact mirror of ROUNDUP: magnitude SHRINKS, sign preserved.
        assert_eq!(excel_rounddown(3.999, 2), 3.99);
        assert_eq!(excel_rounddown(-3.999, 2), -3.99);
        // An exactly-representable decimal is NOT spuriously dropped a step —
        // this is why the epsilon PUSHES away from zero before truncation.
        assert_eq!(excel_rounddown(3.01, 2), 3.01);
        assert_eq!(excel_rounddown(-3.01, 2), -3.01);
        assert_eq!(excel_rounddown(0.0, 2), 0.0);
    }

    #[test]
    fn rounddown_to_negative_digits() {
        assert_eq!(excel_rounddown(1234.0, -2), 1200.0);
        assert_eq!(excel_rounddown(1299.0, -2), 1200.0);
        assert_eq!(excel_rounddown(-1299.0, -2), -1200.0);
    }

    #[test]
    fn rounddown_passes_non_finite_through_unchanged() {
        // Same guard as `excel_roundup` — the caller maps non-finite to an
        // Excel error above this layer.
        assert!(excel_rounddown(f64::NAN, 2).is_nan());
        assert_eq!(excel_rounddown(f64::INFINITY, 2), f64::INFINITY);
        assert_eq!(excel_rounddown(f64::NEG_INFINITY, 2), f64::NEG_INFINITY);
    }

    // PROPERTY tests (CLAUDE.md ALWAYS requirement). The generated magnitudes are
    // deliberately bounded: `ROUND_EPSILON` is a RELATIVE nudge, so once
    // `|x * 10^digits|` approaches `1/ROUND_EPSILON` the nudge exceeds one whole
    // unit at the truncation boundary and both invariants below stop holding for
    // arithmetic reasons that have nothing to do with the rounding rule. The
    // bounds `|x| <= 1000` and `|digits| <= 5` keep `|scaled| <= 1e8`, two orders
    // of magnitude inside `1/(2 * ROUND_EPSILON)`.
    proptest::proptest! {
        #[test]
        fn prop_rounddown_never_grows_the_magnitude(
            x in -1000.0f64..1000.0f64,
            digits in -5i32..=5i32,
        ) {
            let out = excel_rounddown(x, digits);
            proptest::prop_assert!(out.is_finite(), "rounddown({x}, {digits}) = {out} is non-finite");
            // The relative epsilon push can carry the result at most a factor of
            // (1 + ROUND_EPSILON) past |x|; 1e-8 covers that plus the division.
            proptest::prop_assert!(
                out.abs() <= x.abs() * (1.0 + 1e-8) + f64::EPSILON,
                "rounddown({x}, {digits}) = {out} grew the magnitude"
            );
        }

        #[test]
        fn prop_rounddown_magnitude_never_exceeds_roundup(
            x in -1000.0f64..1000.0f64,
            digits in -5i32..=5i32,
        ) {
            let down = excel_rounddown(x, digits);
            let up = excel_roundup(x, digits);
            proptest::prop_assert!(
                down.abs() <= up.abs(),
                "rounddown({x}, {digits}) = {down} exceeds roundup = {up}"
            );
        }
    }

    #[test]
    fn ceiling_positive_rounds_up_to_multiple() {
        assert_eq!(excel_ceiling(10.0, 3.0), 12.0);
        assert_eq!(excel_ceiling(12.0, 3.0), 12.0); // exact multiple unchanged
                                                    // The coil-band CEILING(req*1.05, 50) lands on the next 50.
        let req = 666.0_f64; // req*1.05 = 699.3
        assert_eq!(excel_ceiling(req * 1.05, 50.0), 700.0);
    }

    #[test]
    fn ceiling_negative_magnitude_away_from_zero() {
        // CEILING(-10, -3) == -12 (Excel away-from-zero magnitude rule).
        assert_eq!(excel_ceiling(-10.0, -3.0), -12.0);
        assert_eq!(excel_ceiling(-12.0, -3.0), -12.0);
    }

    #[test]
    fn ceiling_zero_significance_is_zero() {
        assert_eq!(excel_ceiling(10.0, 0.0), 0.0);
    }

    #[test]
    fn ceiling_sign_mismatch_is_nan() {
        assert!(excel_ceiling(10.0, -3.0).is_nan());
        assert!(excel_ceiling(-10.0, 3.0).is_nan());
    }
}
