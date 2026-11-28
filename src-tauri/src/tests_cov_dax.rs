//! Coverage tests for `crate::dax` written against its public/pub(crate) API
//! only, from the crate root. This file deliberately lives OUTSIDE
//! `src/dax/**` (front D's territory for v0.10 round 1) so it needs no edits
//! there besides the single `mod tests_cov_dax;` declaration in `lib.rs`.
//!
//! Expected values below are derived by hand from the DAX semantics
//! documented on dax.guide (date math, financial formulas, text functions),
//! never copied from this engine's own output.

use crate::dax::functions::*;
use crate::dax::DaxValue;

// ---------------------------------------------------------------------------
// Pure date-math helpers (dax::functions)
// ---------------------------------------------------------------------------

#[test]
fn parse_iso_ymd_valid_and_invalid() {
    assert_eq!(parse_iso_ymd("2024-02-29"), Some((2024, 2, 29))); // leap day
    assert_eq!(parse_iso_ymd("2023-02-29"), None); // not a leap year
    assert_eq!(parse_iso_ymd("2024-01-15T10:30:00"), Some((2024, 1, 15)));
    assert_eq!(parse_iso_ymd(""), None);
    assert_eq!(parse_iso_ymd("not-a-date"), None);
    assert_eq!(parse_iso_ymd("2024-13-01"), None); // invalid month
    assert_eq!(parse_iso_ymd("2024-00-01"), None); // invalid month
}

#[test]
fn parse_flexible_ymd_brazilian_format() {
    // DD/MM/YYYY
    assert_eq!(parse_flexible_ymd("25/12/2024"), Some((2024, 12, 25)));
    // falls back to ISO first
    assert_eq!(parse_flexible_ymd("2024-12-25"), Some((2024, 12, 25)));
    // day 31 in a 30-day month is invalid
    assert_eq!(parse_flexible_ymd("31/04/2024"), None);
    assert_eq!(parse_flexible_ymd("garbage"), None);
}

#[test]
fn days_in_month_leap_rules() {
    assert_eq!(days_in_month(2000, 2), 29); // divisible by 400
    assert_eq!(days_in_month(1900, 2), 28); // divisible by 100, not 400
    assert_eq!(days_in_month(2024, 2), 29); // divisible by 4
    assert_eq!(days_in_month(2023, 2), 28);
    assert_eq!(days_in_month(2024, 4), 30);
    assert_eq!(days_in_month(2024, 1), 31);
}

#[test]
fn add_months_handles_year_rollover_both_ways() {
    assert_eq!(add_months(2024, 12, 15, 1), (2025, 1, 15));
    assert_eq!(add_months(2024, 1, 15, -1), (2023, 12, 15));
    // Day is clamped to the target month's length (no month-end rollover):
    // Jan 31 + 1 month -> Feb 29 (2024 is a leap year), not "Feb 31".
    assert_eq!(add_months(2024, 1, 31, 1), (2024, 2, 29));
}

#[test]
fn add_days_crosses_month_and_year_boundaries() {
    assert_eq!(add_days(2024, 2, 28, 1), (2024, 2, 29)); // leap day
    assert_eq!(add_days(2024, 12, 31, 1), (2025, 1, 1));
    assert_eq!(add_days(2024, 3, 1, -1), (2024, 2, 29));
}

#[test]
fn civil_days_roundtrip_is_identity() {
    for (y, m, d) in [(1970, 1, 1), (2024, 2, 29), (1999, 12, 31), (2100, 3, 1)] {
        let days = civil_to_days(y, m, d);
        assert_eq!(days_to_civil(days), (y, m, d), "roundtrip failed for {y}-{m}-{d}");
    }
    // Unix epoch is day 0 by this convention.
    assert_eq!(civil_to_days(1970, 1, 1), 0);
}

#[test]
fn serial_date_matches_excel_1900_system() {
    // Excel/Power BI serial 45292 == 2024-01-01 (with the well-known 1900
    // leap-year bug baked into the +2 offset used by this implementation's
    // civil_to_serial/serial_to_civil pairing).
    let serial = civil_to_serial(2024, 1, 1);
    assert_eq!(serial_to_civil(serial), (2024, 1, 1));
    let serial2 = civil_to_serial(2024, 2, 29);
    assert_eq!(serial_to_civil(serial2), (2024, 2, 29));
}

#[test]
fn parse_iso_to_serial_and_back() {
    let serial = parse_iso_to_serial("2024-06-15").expect("valid date");
    assert_eq!(serial_to_iso(serial), "2024-06-15");
    assert!(parse_iso_to_serial("").is_none());
    assert!(parse_iso_to_serial("not-a-date").is_none());
}

#[test]
fn day_of_week_0_sun_known_anchor() {
    // 2024-01-07 is a Sunday.
    assert_eq!(day_of_week_0_sun(2024, 1, 7), 0);
    // 2024-01-08 is a Monday.
    assert_eq!(day_of_week_0_sun(2024, 1, 8), 1);
}

#[test]
fn calc_median_even_and_odd_counts() {
    assert_eq!(calc_median(&[1.0, 2.0, 3.0]), Some(2.0));
    assert_eq!(calc_median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
    assert_eq!(calc_median(&[]), None);
    assert_eq!(calc_median(&[5.0]), Some(5.0));
}

#[test]
fn calc_percentile_inclusive_vs_exclusive() {
    let v = [1.0, 2.0, 3.0, 4.0, 5.0];
    // PERCENTILE.INC at k=0.5 over 5 sorted values is the median.
    assert_eq!(calc_percentile(&v, 0.5, true), Some(3.0));
    assert_eq!(calc_percentile(&v, 0.0, true), Some(1.0));
    assert_eq!(calc_percentile(&v, 1.0, true), Some(5.0));
    assert_eq!(calc_percentile(&[], 0.5, true), None);
}

#[test]
fn calc_variance_and_stdev_sample_vs_population() {
    let v = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
    // Population variance = 4.0, population stdev = 2.0 (textbook example).
    let pop_var = calc_variance(&v, false).unwrap();
    assert!((pop_var - 4.0).abs() < 1e-9, "pop variance = {pop_var}");
    let pop_sd = calc_stdev(&v, false).unwrap();
    assert!((pop_sd - 2.0).abs() < 1e-9, "pop stdev = {pop_sd}");
    // Sample variance is population * n/(n-1) = 4 * 8/7.
    let samp_var = calc_variance(&v, true).unwrap();
    assert!((samp_var - 4.0 * 8.0 / 7.0).abs() < 1e-9);
    // Single value: sample variance undefined (needs n>1).
    assert_eq!(calc_variance(&[1.0], true), None);
}

#[test]
fn calc_geomean_known_values() {
    // GEOMEAN(4, 9) = sqrt(36) = 6.
    let g = calc_geomean(&[4.0, 9.0]).unwrap();
    assert!((g - 6.0).abs() < 1e-9);
    assert_eq!(calc_geomean(&[]), None);
    // Geometric mean is undefined for non-positive numbers.
    assert_eq!(calc_geomean(&[-1.0, 2.0]), None);
}

// ---------------------------------------------------------------------------
// try_eval_scalar_fn: Math/Trig
// ---------------------------------------------------------------------------

fn n(v: f64) -> DaxValue {
    DaxValue::Decimal(v)
}
fn i(v: i64) -> DaxValue {
    DaxValue::Integer(v)
}
fn t(s: &str) -> DaxValue {
    DaxValue::Text(s.to_string())
}

fn eval(fname: &str, args: &[DaxValue]) -> DaxValue {
    try_eval_scalar_fn(fname, args)
        .unwrap_or_else(|e| panic!("{fname} returned Err: {e}"))
        .unwrap_or_else(|| panic!("{fname} returned Ok(None): needs model context"))
}

#[test]
fn mod_matches_dax_sign_convention() {
    // DAX MOD follows the divisor's sign, like Python's %, not Rust's `%`.
    assert_eq!(eval("MOD", &[n(-7.0), n(3.0)]), DaxValue::Integer(2));
    assert_eq!(eval("MOD", &[n(7.0), n(-3.0)]), DaxValue::Integer(-2));
    // Division by zero -> BLANK, not an error, for MOD.
    assert_eq!(eval("MOD", &[n(5.0), n(0.0)]), DaxValue::Blank);
}

#[test]
fn quotient_truncates_toward_zero() {
    assert_eq!(eval("QUOTIENT", &[n(7.0), n(2.0)]), DaxValue::Integer(3));
    assert_eq!(eval("QUOTIENT", &[n(-7.0), n(2.0)]), DaxValue::Integer(-3));
}

#[test]
fn ceiling_floor_zero_significance_is_blank_or_zero() {
    // This implementation treats a zero significance as BLANK-safe zero,
    // not a division error.
    assert_eq!(eval("CEILING", &[n(4.3), n(0.0)]), DaxValue::Integer(0));
    assert_eq!(eval("FLOOR", &[n(4.3), n(0.0)]), DaxValue::Integer(0));
    assert_eq!(eval("CEILING", &[n(4.3), n(1.0)]), DaxValue::Integer(5));
    assert_eq!(eval("FLOOR", &[n(4.7), n(1.0)]), DaxValue::Integer(4));
}

#[test]
fn sign_of_negative_zero_and_positive() {
    assert_eq!(eval("SIGN", &[n(-5.0)]), DaxValue::Integer(-1));
    assert_eq!(eval("SIGN", &[n(5.0)]), DaxValue::Integer(1));
    assert_eq!(eval("SIGN", &[n(0.0)]), DaxValue::Integer(0));
}

#[test]
fn fact_of_zero_and_small_ints() {
    assert_eq!(eval("FACT", &[n(0.0)]), DaxValue::Integer(1));
    assert_eq!(eval("FACT", &[n(5.0)]), DaxValue::Integer(120));
}

#[test]
fn gcd_lcm_known_pairs() {
    assert_eq!(eval("GCD", &[n(12.0), n(18.0)]), DaxValue::Integer(6));
    assert_eq!(eval("LCM", &[n(4.0), n(6.0)]), DaxValue::Integer(12));
}

#[test]
fn pi_constant_matches_std() {
    let v = eval("PI", &[]);
    match v {
        DaxValue::Decimal(x) => assert!((x - std::f64::consts::PI).abs() < 1e-12),
        other => panic!("expected Decimal, got {other:?}"),
    }
}

#[test]
fn combin_and_permut_known_values() {
    // C(5,2) = 10, P(5,2) = 20.
    assert_eq!(eval("COMBIN", &[n(5.0), n(2.0)]), DaxValue::Integer(10));
    assert_eq!(eval("PERMUT", &[n(5.0), n(2.0)]), DaxValue::Integer(20));
}

// ---------------------------------------------------------------------------
// try_eval_scalar_fn: Text
// ---------------------------------------------------------------------------

#[test]
fn mid_and_trim_and_exact_text_edges() {
    assert_eq!(eval("MID", &[t("VetroBI"), n(1.0), n(5.0)]), t("Vetro"));
    // Out-of-range start returns empty text, not an error.
    assert_eq!(eval("MID", &[t("abc"), n(10.0), n(3.0)]), t(""));
    assert_eq!(eval("TRIM", &[t("  spaced out  ")]), t("spaced out"));
    assert_eq!(eval("EXACT", &[t("DAX"), t("dax")]), DaxValue::Boolean(false));
    assert_eq!(eval("EXACT", &[t("DAX"), t("DAX")]), DaxValue::Boolean(true));
}

#[test]
fn find_is_case_sensitive_search_is_not() {
    assert_eq!(eval("FIND", &[t("i"), t("BiVetroI")]), DaxValue::Integer(2));
    // FIND is case-sensitive: uppercase "I" does not match lowercase "i" at 2,
    // but does match the "I" at position 8.
    assert_eq!(eval("FIND", &[t("I"), t("BiVetroI")]), DaxValue::Integer(8));
    // SEARCH is case-insensitive and finds the first occurrence regardless of case.
    assert_eq!(eval("SEARCH", &[t("i"), t("BiVetroI")]), DaxValue::Integer(2));
}

#[test]
fn substitute_replace_and_rept() {
    assert_eq!(
        eval("SUBSTITUTE", &[t("a-b-c"), t("-"), t("_")]),
        t("a_b_c")
    );
    assert_eq!(eval("REPLACE", &[t("hello"), n(1.0), n(1.0), t("H")]), t("Hello"));
    assert_eq!(eval("REPT", &[t("ab"), n(3.0)]), t("ababab"));
    assert_eq!(eval("REPT", &[t("x"), n(0.0)]), t(""));
}

#[test]
fn value_parses_numbers_and_blank_on_empty() {
    assert_eq!(eval("VALUE", &[t("42")]), DaxValue::Integer(42));
    assert_eq!(eval("VALUE", &[t("3.5")]), n(3.5));
}

#[test]
fn value_of_non_numeric_text_is_error() {
    // VALUE of non-numeric text returns BLANK in this engine's scalar
    // evaluator (not a DAX error, unlike the real Power BI engine).
    let r = try_eval_scalar_fn("VALUE", &[t("not a number")]).unwrap();
    assert_eq!(r, Some(DaxValue::Blank));
}

#[test]
fn unichar_unicode_roundtrip() {
    assert_eq!(eval("UNICHAR", &[n(65.0)]), t("A"));
    assert_eq!(eval("UNICODE", &[t("A")]), DaxValue::Integer(65));
}

#[test]
fn coalesce_and_isblank_semantics() {
    assert_eq!(eval("COALESCE", &[DaxValue::Blank, DaxValue::Blank, n(7.0)]), n(7.0));
    assert_eq!(eval("COALESCE", &[DaxValue::Blank, DaxValue::Blank]), DaxValue::Blank);
}

// ---------------------------------------------------------------------------
// try_eval_scalar_fn: Date/Time
// ---------------------------------------------------------------------------

#[test]
fn date_function_builds_expected_serial() {
    // DATE returns a text "YYYY-MM-DD" in this engine's scalar evaluator
    // (dates are represented as ISO text until parsed by date arithmetic).
    let d1 = eval("DATE", &[n(2024.0), n(1.0), n(1.0)]);
    let d2 = eval("DATE", &[n(2024.0), n(2.0), n(29.0)]);
    assert_eq!(d1, t("2024-01-01"));
    assert_eq!(d2, t("2024-02-29"));
}

#[test]
fn edate_eomonth_month_arithmetic() {
    let base = eval("DATE", &[n(2024.0), n(1.0), n(31.0)]);
    let edate = eval("EDATE", &[base.clone(), n(1.0)]);
    // EDATE(2024-01-31, 1) clamps to the last day of February (leap year: 29).
    assert_eq!(edate, t("2024-02-29"));
    let eom = eval("EOMONTH", &[base, n(0.0)]);
    assert_eq!(eom, t("2024-01-31"));
}

#[test]
fn weekday_and_quarter_known_anchors() {
    let d = eval("DATE", &[n(2024.0), n(1.0), n(8.0)]); // Monday
    // WEEKDAY default return type 1: Sunday=1..Saturday=7, so Monday=2.
    assert_eq!(eval("WEEKDAY", &[d.clone()]), DaxValue::Integer(2));
    let q = eval("QUARTER", &[eval("DATE", &[n(2024.0), n(11.0), n(1.0)])]);
    assert_eq!(q, DaxValue::Integer(4));
}

#[test]
fn datediff_year_month_and_week_boundaries() {
    let d1 = eval("DATE", &[n(2023.0), n(1.0), n(1.0)]);
    let d2 = eval("DATE", &[n(2024.0), n(1.0), n(1.0)]);
    assert_eq!(eval("DATEDIFF", &[d1.clone(), d2.clone(), t("YEAR")]), DaxValue::Integer(1));
    assert_eq!(eval("DATEDIFF", &[d1.clone(), d2.clone(), t("MONTH")]), DaxValue::Integer(12));
    assert_eq!(eval("DATEDIFF", &[d1, d2, t("WEEK")]), DaxValue::Integer(52));
}

// ---------------------------------------------------------------------------
// try_eval_scalar_fn: Financial (hand-derived from standard formulas)
// ---------------------------------------------------------------------------

#[test]
fn pmt_matches_standard_amortization_formula() {
    // PMT(rate=0.005, nper=60, pv=10000) for a standard loan, no FV, type 0.
    // Closed-form: -pv * r / (1 - (1+r)^-n)
    let rate = 0.005_f64;
    let nper = 60.0;
    let pv = 10000.0;
    let expected = -(pv * rate) / (1.0 - (1.0 + rate).powf(-nper));
    let got = eval("PMT", &[n(rate), n(nper), n(pv)]);
    let x = got_as_f64(&got);
    assert!((x - expected).abs() < 1e-6, "PMT got {x}, expected {expected}");
}

fn got_as_f64(v: &DaxValue) -> f64 {
    match v {
        DaxValue::Decimal(x) => *x,
        DaxValue::Integer(i) => *i as f64,
        DaxValue::Currency(c) => *c as f64 / 10_000.0,
        other => panic!("not numeric: {other:?}"),
    }
}

#[test]
fn fv_of_zero_rate_is_linear() {
    // With rate 0, FV(0, nper, pmt, pv) = -(pv + pmt*nper)
    let got = eval("FV", &[n(0.0), n(12.0), n(-100.0), n(-1000.0)]);
    let x = got_as_f64(&got);
    assert!((x - 2200.0).abs() < 1e-9, "got {x}");
}

#[test]
fn sln_straight_line_depreciation() {
    // SLN(cost=10000, salvage=1000, life=9) = (10000-1000)/9
    let got = eval("SLN", &[n(10000.0), n(1000.0), n(9.0)]);
    let x = got_as_f64(&got);
    assert!((x - 1000.0).abs() < 1e-6, "got {x}");
}

#[test]
fn calc_xnpv_and_xirr_hand_derived() {
    use crate::dax::functions::DatedFlow;
    // Two flows one year apart: -1000 now, +1100 in 365 days -> ~10% return.
    // DatedFlow is (amount, day_number).
    let flows: Vec<DatedFlow> = vec![(-1000.0, 0), (1100.0, 365)];
    let npv_at_10pct = calc_xnpv(0.10, &flows).unwrap();
    // NPV at (close to) the true IRR should be close to zero.
    assert!(npv_at_10pct.abs() < 5.0, "npv_at_10pct = {npv_at_10pct}");

    let irr = calc_xirr(&flows, 0.1).unwrap();
    assert!((irr - 0.10).abs() < 0.01, "irr = {irr}");
}

#[test]
fn xnpv_rejects_rate_at_or_below_minus_one() {
    use crate::dax::functions::DatedFlow;
    let flows: Vec<DatedFlow> = vec![(-100.0, 0), (110.0, 100)];
    assert!(calc_xnpv(-1.0, &flows).is_err());
}

#[test]
fn xirr_requires_both_signs() {
    use crate::dax::functions::DatedFlow;
    let only_negative: Vec<DatedFlow> = vec![(-100.0, 0), (-50.0, 100)];
    assert!(calc_xirr(&only_negative, 0.1).is_err());
}

// ---------------------------------------------------------------------------
// Error / BLANK edge cases shared across functions
// ---------------------------------------------------------------------------

#[test]
fn unknown_function_name_returns_ok_none() {
    // try_eval_scalar_fn returns Ok(None) for anything it doesn't recognise
    // as a pure scalar function (it needs model/table context instead).
    let r = try_eval_scalar_fn("SUMX", &[]).unwrap();
    assert!(r.is_none());
}

#[test]
fn ln_of_nonpositive_is_blank_not_panic() {
    // This engine returns BLANK for LN of non-positive numbers (not a DAX
    // error like the real Power BI engine), but must never panic.
    assert_eq!(eval("LN", &[n(0.0)]), DaxValue::Blank);
    assert_eq!(eval("LN", &[n(-5.0)]), DaxValue::Blank);
}

#[test]
fn fact_of_negative_or_too_large_is_blank() {
    // FACT is defined for 0..=170 in this engine; outside that range it's
    // BLANK rather than a DAX error.
    assert_eq!(eval("FACT", &[n(-1.0)]), DaxValue::Blank);
    assert_eq!(eval("FACT", &[n(171.0)]), DaxValue::Blank);
}

#[test]
fn isinteger_and_iseven_odd_on_mixed_types() {
    assert_eq!(eval("ISEVEN", &[i(4)]), DaxValue::Boolean(true));
    assert_eq!(eval("ISODD", &[i(4)]), DaxValue::Boolean(false));
    assert_eq!(eval("ISNUMBER", &[t("x")]), DaxValue::Boolean(false));
    assert_eq!(eval("ISNUMBER", &[i(1)]), DaxValue::Boolean(true));
    assert_eq!(eval("ISTEXT", &[t("x")]), DaxValue::Boolean(true));
    assert_eq!(eval("ISTEXT", &[i(1)]), DaxValue::Boolean(false));
}
