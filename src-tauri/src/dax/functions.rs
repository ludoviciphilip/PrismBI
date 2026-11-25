use crate::dax::DaxValue;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Parses YYYY-MM-DD (optionally with `T...` or space time suffix) into `(year, month, day)`.
pub fn parse_iso_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let date_part = trimmed.split(['T', ' ']).next()?.trim();
    let mut parts = date_part.split('-');
    let y = parts.next()?.parse::<i32>().ok()?;
    let m = parts.next()?.parse::<u32>().ok()?;
    let d = parts.next()?.parse::<u32>().ok()?;
    if (1..=12).contains(&m) && (1..=days_in_month(y, m)).contains(&d) {
        Some((y, m, d))
    } else {
        None
    }
}

/// Parses either ISO `YYYY-MM-DD` or `DD/MM/YYYY` into `(year, month, day)`.
pub fn parse_flexible_ymd(s: &str) -> Option<(i32, u32, u32)> {
    if let Some(ymd) = parse_iso_ymd(s) {
        return Some(ymd);
    }
    let date_part = s.trim().split(['T', ' ']).next()?.trim();
    let slash_parts: Vec<&str> = date_part.split('/').collect();
    if slash_parts.len() == 3 {
        let a = slash_parts[0].parse::<i32>().ok()?;
        let b = slash_parts[1].parse::<u32>().ok()?;
        let c = slash_parts[2].parse::<i32>().ok()?;
        if c >= 1000 && (1..=12).contains(&b) && a >= 1 && (a as u32) <= days_in_month(c, b) {
            return Some((c, b, a as u32));
        }
    }
    None
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

pub fn add_months(y: i32, m: u32, d: u32, delta_months: i32) -> (i32, u32, u32) {
    let total_m = y * 12 + (m as i32 - 1) + delta_months;
    let ny = total_m.div_euclid(12);
    let nm = (total_m.rem_euclid(12) + 1) as u32;
    let max_d = days_in_month(ny, nm);
    (ny, nm, d.min(max_d))
}

pub fn add_days(y: i32, m: u32, d: u32, delta_days: i32) -> (i32, u32, u32) {
    let days = civil_to_days(y, m, d) + delta_days;
    days_to_civil(days)
}

pub fn civil_to_days(y: i32, m: u32, d: u32) -> i32 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let d = d as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe as i64 - 719468) as i32
}

pub fn days_to_civil(days: i32) -> (i32, u32, u32) {
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

pub fn today_ymd() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i32;
    days_to_civil(days)
}

pub fn day_of_week_0_sun(y: i32, m: u32, d: u32) -> u32 {
    // 1970-01-01 was Thursday (4)
    let days = civil_to_days(y, m, d);
    (days + 4).rem_euclid(7) as u32
}

pub fn calc_median(vals: &[f64]) -> Option<f64> {
    if vals.is_empty() {
        return None;
    }
    let mut sorted = vals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        Some(sorted[n / 2])
    } else {
        Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0)
    }
}

pub fn calc_percentile(vals: &[f64], k: f64, inclusive: bool) -> Option<f64> {
    if vals.is_empty() || !k.is_finite() {
        return None;
    }
    if inclusive && !(0.0..=1.0).contains(&k) {
        return None;
    }
    if !inclusive && !(k > 0.0 && k < 1.0) {
        return None;
    }
    let mut sorted = vals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n == 1 {
        return Some(sorted[0]);
    }
    let rank = if inclusive {
        k * (n as f64 - 1.0)
    } else {
        k * (n as f64 + 1.0) - 1.0
    };
    if rank <= 0.0 {
        return Some(sorted[0]);
    }
    if rank >= (n - 1) as f64 {
        return Some(sorted[n - 1]);
    }
    let lo = rank.floor() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] + frac * (sorted[lo + 1] - sorted[lo]))
}

pub fn calc_variance(vals: &[f64], sample: bool) -> Option<f64> {
    if vals.is_empty() || (sample && vals.len() < 2) {
        return None;
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    let ss: f64 = vals.iter().map(|v| (v - mean).powi(2)).sum();
    let denom = if sample { (vals.len() - 1) as f64 } else { vals.len() as f64 };
    Some(ss / denom)
}

pub fn calc_stdev(vals: &[f64], sample: bool) -> Option<f64> {
    calc_variance(vals, sample).map(|v| v.sqrt())
}

pub fn calc_geomean(vals: &[f64]) -> Option<f64> {
    if vals.is_empty() || vals.iter().any(|&v| v <= 0.0) {
        return None;
    }
    let avg_ln = vals.iter().map(|v| v.ln()).sum::<f64>() / vals.len() as f64;
    Some(avg_ln.exp())
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn pseudo_random_0_1(seed_extra: usize) -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1234567);
    let mut h = DefaultHasher::new();
    nanos.hash(&mut h);
    seed_extra.hash(&mut h);
    let v = h.finish() % 1_000_000;
    (v as f64) / 1_000_000.0
}

fn round_with_decimals(val: f64, digits: i32, mode: i8) -> f64 {
    // mode: 1 = away from zero (ROUNDUP), -1 = toward zero (ROUNDDOWN)
    let factor = 10f64.powi(digits);
    let scaled = val * factor;
    let rounded = if mode > 0 {
        if scaled >= 0.0 {
            scaled.ceil()
        } else {
            scaled.floor()
        }
    } else if scaled >= 0.0 {
        scaled.floor()
    } else {
        scaled.ceil()
    };
    rounded / factor
}

fn format_number_grouped(val: f64, decimals: usize) -> String {
    let neg = val < 0.0;
    let abs = val.abs();
    let raw = format!("{:.prec$}", abs, prec = decimals);
    let mut parts = raw.split('.');
    let int_part = parts.next().unwrap_or("0");
    let frac_part = parts.next();

    let mut grouped = String::new();
    for (idx, ch) in int_part.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let int_fmt: String = grouped.chars().rev().collect();
    let sign = if neg { "-" } else { "" };
    if let Some(frac) = frac_part {
        if decimals > 0 {
            return format!("{}{}.{}", sign, int_fmt, frac);
        }
    }
    format!("{}{}", sign, int_fmt)
}

fn format_dax_value(val: &DaxValue, pattern: &str) -> String {
    let p_trimmed = pattern.trim();
    let lower = p_trimmed.to_lowercase();

    // Try date formatting first if pattern looks like a date format
    if matches!(val, DaxValue::Text(_))
        && (lower.contains('y') || lower.contains('m') || lower.contains('d'))
    {
        if let Some((y, m, d)) = parse_flexible_ymd(&val.as_display_string()) {
            const MONTHS_PT: [&str; 12] = [
                "janeiro", "fevereiro", "março", "abril", "maio", "junho",
                "julho", "agosto", "setembro", "outubro", "novembro", "dezembro",
            ];
            const MONTHS_SHORT_PT: [&str; 12] = [
                "Jan", "Fev", "Mar", "Abr", "Mai", "Jun",
                "Jul", "Ago", "Set", "Out", "Nov", "Dez",
            ];
            let m_idx = (m as usize).saturating_sub(1).min(11);
            let mut out = p_trimmed.to_string();
            out = out.replace("yyyy", &format!("{:04}", y));
            out = out.replace("YYYY", &format!("{:04}", y));
            out = out.replace("yy", &format!("{:02}", y.rem_euclid(100)));
            out = out.replace("MMMM", MONTHS_PT[m_idx]);
            out = out.replace("mmmm", MONTHS_PT[m_idx]);
            out = out.replace("MMM", MONTHS_SHORT_PT[m_idx]);
            out = out.replace("mmm", MONTHS_SHORT_PT[m_idx]);
            out = out.replace("MM", &format!("{:02}", m));
            out = out.replace("mm", &format!("{:02}", m));
            out = out.replace("dd", &format!("{:02}", d));
            out = out.replace("DD", &format!("{:02}", d));
            return out;
        }
    }

    let num = match val.as_f64() {
        Some(n) if !val.is_blank() => n,
        _ => return val.as_display_string(),
    };

    if lower == "currency" || p_trimmed.contains("R$") {
        return format!("R$ {}", format_number_grouped(num, 2));
    }
    if p_trimmed.contains('$') {
        return format!("$ {}", format_number_grouped(num, 2));
    }
    if lower == "percent" || p_trimmed.contains('%') {
        let decimals = if p_trimmed.contains('.') {
            p_trimmed
                .split('.')
                .nth(1)
                .map(|s| s.chars().filter(|&c| c == '0' || c == '#').count())
                .unwrap_or(0)
        } else if lower == "percent" {
            2
        } else {
            0
        };
        return format!("{:.prec$}%", num * 100.0, prec = decimals);
    }
    if lower == "standard" || lower == "fixed" || p_trimmed.contains('#') || p_trimmed.contains('0') {
        let decimals = p_trimmed
            .split('.')
            .nth(1)
            .map(|s| s.chars().filter(|&c| c == '0' || c == '#').count())
            .unwrap_or(if lower == "fixed" || lower == "standard" { 2 } else { 0 });
        return format_number_grouped(num, decimals);
    }

    val.as_display_string()
}

/// Tokens for a simplified wildcard pattern as used by DAX SEARCH/LIKE:
/// `*` matches any run of characters (incl. empty), `?` matches exactly
/// one character, and `~` escapes the following character as a literal.
enum WildcardToken {
    Star,
    Any,
    Lit(char),
}

fn tokenize_wildcard(pattern: &str) -> Vec<WildcardToken> {
    let mut out = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '~' => {
                if let Some(n) = chars.next() {
                    out.push(WildcardToken::Lit(n));
                } else {
                    out.push(WildcardToken::Lit('~'));
                }
            }
            '*' => out.push(WildcardToken::Star),
            '?' => out.push(WildcardToken::Any),
            other => out.push(WildcardToken::Lit(other)),
        }
    }
    out
}

fn wildcard_match_from(hay: &[char], hi: usize, tok: &[WildcardToken], ti: usize) -> bool {
    if ti == tok.len() {
        return true;
    }
    match &tok[ti] {
        WildcardToken::Lit(c) => hi < hay.len() && hay[hi] == *c && wildcard_match_from(hay, hi + 1, tok, ti + 1),
        WildcardToken::Any => hi < hay.len() && wildcard_match_from(hay, hi + 1, tok, ti + 1),
        WildcardToken::Star => {
            for k in hi..=hay.len() {
                if wildcard_match_from(hay, k, tok, ti + 1) {
                    return true;
                }
            }
            false
        }
    }
}

/// Finds the first char offset in `hay` where `tok` matches (DAX SEARCH semantics:
/// the pattern need not consume the rest of the haystack).
fn wildcard_find(hay: &[char], tok: &[WildcardToken]) -> Option<usize> {
    for start in 0..=hay.len() {
        if wildcard_match_from(hay, start, tok, 0) {
            return Some(start);
        }
    }
    None
}

/// Evaluates pure scalar DAX functions (Math/Trig, Text, Date/Time, Info, Financial).
/// Returns `Ok(Some(value))` when handled, `Ok(None)` when the function needs model/filter context.
pub fn try_eval_scalar_fn(fname: &str, args: &[DaxValue]) -> Result<Option<DaxValue>, String> {
    let arg_f64 = |idx: usize, default: f64| -> f64 {
        args.get(idx)
            .and_then(|v| if v.is_blank() { None } else { v.as_f64() })
            .unwrap_or(default)
    };
    let arg_str = |idx: usize| -> String {
        args.get(idx).map(|v| v.as_display_string()).unwrap_or_default()
    };

    let res = match fname {
        // ==========================================
        // Information & Conversion (scalar)
        // ==========================================
        "ISNUMBER" | "ISNUMERIC" | "ISDOUBLE" | "ISCURRENCY" => {
            DaxValue::Boolean(matches!(args.first(), Some(DaxValue::Number(_))))
        }
        "ISINT64" => DaxValue::Boolean(match args.first() {
            Some(DaxValue::Number(n)) => n.is_finite() && (n.fract()).abs() < 1e-12,
            _ => false,
        }),
        "ISTEXT" | "ISSTRING" => DaxValue::Boolean(matches!(args.first(), Some(DaxValue::Text(_)))),
        "ISLOGICAL" | "ISBOOLEAN" => {
            DaxValue::Boolean(matches!(args.first(), Some(DaxValue::Boolean(_))))
        }
        "ISDATETIME" => DaxValue::Boolean(match args.first() {
            Some(DaxValue::Text(s)) => parse_flexible_ymd(s).is_some(),
            _ => false,
        }),
        "ISNONTEXT" => DaxValue::Boolean(!matches!(args.first(), Some(DaxValue::Text(_)))),
        "ISERROR" => DaxValue::Boolean(false),
        "ISEVEN" => DaxValue::Boolean((arg_f64(0, 0.0).trunc() as i64).rem_euclid(2) == 0),
        "ISODD" => DaxValue::Boolean((arg_f64(0, 0.0).trunc() as i64).rem_euclid(2) != 0),
        "COALESCE" => args
            .iter()
            .find(|v| !v.is_blank())
            .cloned()
            .unwrap_or(DaxValue::Blank),

        // ==========================================
        // Math & Trigonometry
        // ==========================================
        "ROUNDUP" => {
            let n = arg_f64(0, 0.0);
            let d = arg_f64(1, 0.0).trunc() as i32;
            DaxValue::Number(round_with_decimals(n, d, 1))
        }
        "ROUNDDOWN" => {
            let n = arg_f64(0, 0.0);
            let d = arg_f64(1, 0.0).trunc() as i32;
            DaxValue::Number(round_with_decimals(n, d, -1))
        }
        "TRUNC" => {
            let n = arg_f64(0, 0.0);
            let d = arg_f64(1, 0.0).trunc() as i32;
            DaxValue::Number(round_with_decimals(n, d, -1))
        }
        "MROUND" => {
            let n = arg_f64(0, 0.0);
            let mult = arg_f64(1, 1.0);
            if mult.abs() < 1e-12 {
                DaxValue::Number(0.0)
            } else {
                DaxValue::Number((n / mult).round() * mult)
            }
        }
        "CEILING" | "ISO.CEILING" => {
            let n = arg_f64(0, 0.0);
            let sig = arg_f64(1, 1.0);
            if sig.abs() < 1e-12 {
                DaxValue::Number(0.0)
            } else {
                DaxValue::Number((n / sig).ceil() * sig)
            }
        }
        "FLOOR" => {
            let n = arg_f64(0, 0.0);
            let sig = arg_f64(1, 1.0);
            if sig.abs() < 1e-12 {
                DaxValue::Number(0.0)
            } else {
                DaxValue::Number((n / sig).floor() * sig)
            }
        }
        "MOD" => {
            let n = arg_f64(0, 0.0);
            let d = arg_f64(1, 1.0);
            if d.abs() < 1e-12 {
                DaxValue::Blank
            } else {
                DaxValue::Number(n - d * (n / d).floor())
            }
        }
        "QUOTIENT" => {
            let n = arg_f64(0, 0.0);
            let d = arg_f64(1, 1.0);
            if d.abs() < 1e-12 {
                DaxValue::Blank
            } else {
                DaxValue::Number((n / d).trunc())
            }
        }
        "SIGN" => {
            let n = arg_f64(0, 0.0);
            let s = if n > 0.0 { 1.0 } else if n < 0.0 { -1.0 } else { 0.0 };
            DaxValue::Number(s)
        }
        "EVEN" => {
            let n = arg_f64(0, 0.0);
            let v = if n >= 0.0 {
                (n / 2.0).ceil() * 2.0
            } else {
                (n / 2.0).floor() * 2.0
            };
            DaxValue::Number(v)
        }
        "ODD" => {
            let n = arg_f64(0, 0.0);
            let v = if n == 0.0 {
                1.0
            } else if n > 0.0 {
                let c = n.ceil();
                if (c as i64) % 2 == 0 { c + 1.0 } else { c }
            } else {
                let f = n.floor();
                if (f as i64).abs() % 2 == 0 { f - 1.0 } else { f }
            };
            DaxValue::Number(v)
        }
        "EXP" => DaxValue::Number(arg_f64(0, 0.0).exp()),
        "LN" => {
            let n = arg_f64(0, 0.0);
            if n <= 0.0 { DaxValue::Blank } else { DaxValue::Number(n.ln()) }
        }
        "LOG10" => {
            let n = arg_f64(0, 0.0);
            if n <= 0.0 { DaxValue::Blank } else { DaxValue::Number(n.log10()) }
        }
        "LOG" => {
            let n = arg_f64(0, 0.0);
            let base = arg_f64(1, 10.0);
            if n <= 0.0 || base <= 0.0 || (base - 1.0).abs() < 1e-12 {
                DaxValue::Blank
            } else {
                DaxValue::Number(n.log(base))
            }
        }
        "PI" => DaxValue::Number(std::f64::consts::PI),
        "SQRTPI" => {
            let n = arg_f64(0, 0.0);
            if n < 0.0 { DaxValue::Blank } else { DaxValue::Number((n * std::f64::consts::PI).sqrt()) }
        }
        "FACT" => {
            let n = arg_f64(0, 0.0).floor();
            if n < 0.0 || n > 170.0 {
                DaxValue::Blank
            } else {
                let mut acc = 1.0f64;
                for i in 2..=(n as u64) {
                    acc *= i as f64;
                }
                DaxValue::Number(acc)
            }
        }
        "GCD" => {
            let a = arg_f64(0, 0.0).abs().floor() as u64;
            let b = arg_f64(1, 0.0).abs().floor() as u64;
            DaxValue::Number(gcd_u64(a, b) as f64)
        }
        "LCM" => {
            let a = arg_f64(0, 0.0).abs().floor() as u64;
            let b = arg_f64(1, 0.0).abs().floor() as u64;
            if a == 0 || b == 0 {
                DaxValue::Number(0.0)
            } else {
                let g = gcd_u64(a, b);
                DaxValue::Number(((a / g) * b) as f64)
            }
        }
        "RAND" => DaxValue::Number(pseudo_random_0_1(args.len())),
        "RANDBETWEEN" => {
            let lo = arg_f64(0, 0.0).ceil() as i64;
            let hi = arg_f64(1, 1.0).floor() as i64;
            if hi < lo {
                DaxValue::Blank
            } else {
                let span = (hi - lo + 1) as f64;
                let r = (pseudo_random_0_1(lo as usize ^ hi as usize) * span).floor() as i64;
                DaxValue::Number((lo + r.min(hi - lo)) as f64)
            }
        }
        "SIN" => DaxValue::Number(arg_f64(0, 0.0).sin()),
        "COS" => DaxValue::Number(arg_f64(0, 0.0).cos()),
        "TAN" => DaxValue::Number(arg_f64(0, 0.0).tan()),
        "ASIN" => {
            let n = arg_f64(0, 0.0);
            if (-1.0..=1.0).contains(&n) { DaxValue::Number(n.asin()) } else { DaxValue::Blank }
        }
        "ACOS" => {
            let n = arg_f64(0, 0.0);
            if (-1.0..=1.0).contains(&n) { DaxValue::Number(n.acos()) } else { DaxValue::Blank }
        }
        "ATAN" => DaxValue::Number(arg_f64(0, 0.0).atan()),
        "DEGREES" => DaxValue::Number(arg_f64(0, 0.0).to_degrees()),
        "RADIANS" => DaxValue::Number(arg_f64(0, 0.0).to_radians()),
        "CURRENCY" => DaxValue::Number((arg_f64(0, 0.0) * 10_000.0).round() / 10_000.0),
        "BITAND" => DaxValue::Number(((arg_f64(0, 0.0) as i64) & (arg_f64(1, 0.0) as i64)) as f64),
        "BITOR" => DaxValue::Number(((arg_f64(0, 0.0) as i64) | (arg_f64(1, 0.0) as i64)) as f64),
        "BITXOR" => DaxValue::Number(((arg_f64(0, 0.0) as i64) ^ (arg_f64(1, 0.0) as i64)) as f64),
        "BITLSHIFT" => {
            let n = arg_f64(0, 0.0) as i64;
            let shift = (arg_f64(1, 0.0) as i32).clamp(-63, 63);
            let shifted = if shift >= 0 {
                n.wrapping_shl(shift as u32)
            } else {
                n.wrapping_shr((-shift) as u32)
            };
            DaxValue::Number(shifted as f64)
        }
        "BITRSHIFT" => {
            let n = arg_f64(0, 0.0) as i64;
            let shift = (arg_f64(1, 0.0) as i32).clamp(-63, 63);
            let shifted = if shift >= 0 {
                n.wrapping_shr(shift as u32)
            } else {
                n.wrapping_shl((-shift) as u32)
            };
            DaxValue::Number(shifted as f64)
        }
        "SINH" => DaxValue::Number(arg_f64(0, 0.0).sinh()),
        "COSH" => DaxValue::Number(arg_f64(0, 0.0).cosh()),
        "TANH" => DaxValue::Number(arg_f64(0, 0.0).tanh()),
        "ASINH" => DaxValue::Number(arg_f64(0, 0.0).asinh()),
        "ACOSH" => {
            let n = arg_f64(0, 1.0);
            if n >= 1.0 { DaxValue::Number(n.acosh()) } else { DaxValue::Blank }
        }
        "ATANH" => {
            let n = arg_f64(0, 0.0);
            if n > -1.0 && n < 1.0 { DaxValue::Number(n.atanh()) } else { DaxValue::Blank }
        }
        "COMBIN" => {
            let n = arg_f64(0, 0.0).trunc() as i64;
            let k = arg_f64(1, 0.0).trunc() as i64;
            if n < 0 || k < 0 || k > n {
                DaxValue::Blank
            } else {
                let k = k.min(n - k);
                let mut res = 1.0f64;
                for i in 0..k {
                    res = res * ((n - i) as f64) / ((i + 1) as f64);
                }
                DaxValue::Number(res.round())
            }
        }
        "COMBINA" => {
            let n = arg_f64(0, 0.0).trunc() as i64;
            let k = arg_f64(1, 0.0).trunc() as i64;
            if n <= 0 || k < 0 {
                if n == 0 && k == 0 { DaxValue::Number(1.0) } else { DaxValue::Blank }
            } else {
                let top = n + k - 1;
                let kk = k.min(top - k);
                let mut res = 1.0f64;
                for i in 0..kk {
                    res = res * ((top - i) as f64) / ((i + 1) as f64);
                }
                DaxValue::Number(res.round())
            }
        }
        "PERMUT" => {
            let n = arg_f64(0, 0.0).trunc() as i64;
            let k = arg_f64(1, 0.0).trunc() as i64;
            if n < 0 || k < 0 || k > n {
                DaxValue::Blank
            } else {
                let mut res = 1.0f64;
                for i in 0..k {
                    res *= (n - i) as f64;
                }
                DaxValue::Number(res)
            }
        }
        "CONVERT" => {
            let v = args.first().cloned().unwrap_or(DaxValue::Blank);
            if v.is_blank() {
                DaxValue::Blank
            } else {
                let target_t = arg_str(1).trim().to_uppercase();
                match target_t.as_str() {
                    "INTEGER" | "INT64" => DaxValue::Number(v.as_f64().unwrap_or(0.0).trunc()),
                    "DOUBLE" | "DECIMAL" => DaxValue::Number(v.as_f64().unwrap_or(0.0)),
                    "CURRENCY" => DaxValue::Number((v.as_f64().unwrap_or(0.0) * 10_000.0).round() / 10_000.0),
                    "STRING" | "TEXT" => DaxValue::Text(v.as_display_string()),
                    "BOOLEAN" => DaxValue::Boolean(v.as_bool()),
                    "DATETIME" | "DATE" => {
                        if let Some((y, m, d)) = parse_flexible_ymd(&v.as_display_string()) {
                            DaxValue::Text(format!("{:04}-{:02}-{:02}", y, m, d))
                        } else {
                            DaxValue::Text(v.as_display_string())
                        }
                    }
                    _ => v,
                }
            }
        }

        // ==========================================
        // Text Functions
        // ==========================================
        "MID" => {
            let s: Vec<char> = arg_str(0).chars().collect();
            let start = (arg_f64(1, 1.0).max(1.0) as usize).saturating_sub(1);
            let count = arg_f64(2, 0.0).max(0.0) as usize;
            let out: String = s.into_iter().skip(start).take(count).collect();
            DaxValue::Text(out)
        }
        "TRIM" => {
            let s = arg_str(0);
            let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
            DaxValue::Text(collapsed)
        }
        "FIND" => {
            let needle = arg_str(0);
            let haystack = arg_str(1);
            let start = (arg_f64(2, 1.0).max(1.0) as usize).saturating_sub(1);
            let tail: String = haystack.chars().skip(start).collect();
            if let Some(byte_idx) = tail.find(&needle) {
                let char_pos = tail[..byte_idx].chars().count() + start + 1;
                DaxValue::Number(char_pos as f64)
            } else if args.len() >= 4 {
                args[3].clone()
            } else {
                return Err("FIND: texto não encontrado".to_string());
            }
        }
        "SEARCH" => {
            let needle = arg_str(0).to_lowercase();
            let haystack = arg_str(1).to_lowercase();
            let start = (arg_f64(2, 1.0).max(1.0) as usize).saturating_sub(1);
            let tail_chars: Vec<char> = haystack.chars().skip(start).collect();
            let found = if needle.contains(['*', '?', '~']) {
                let tokens = tokenize_wildcard(&needle);
                wildcard_find(&tail_chars, &tokens)
            } else {
                let tail: String = tail_chars.iter().collect();
                tail.find(&needle).map(|byte_idx| tail[..byte_idx].chars().count())
            };
            if let Some(char_offset) = found {
                DaxValue::Number((char_offset + start + 1) as f64)
            } else if args.len() >= 4 {
                args[3].clone()
            } else {
                DaxValue::Blank
            }
        }
        "SUBSTITUTE" => {
            let text = arg_str(0);
            let old = arg_str(1);
            let new = arg_str(2);
            if old.is_empty() {
                DaxValue::Text(text)
            } else if args.len() >= 4 {
                let nth = arg_f64(3, 1.0).max(1.0) as usize;
                let mut count = 0usize;
                let mut out = String::with_capacity(text.len());
                let mut rest = text.as_str();
                while let Some(idx) = rest.find(&old) {
                    count += 1;
                    out.push_str(&rest[..idx]);
                    if count == nth {
                        out.push_str(&new);
                        out.push_str(&rest[idx + old.len()..]);
                        return Ok(Some(DaxValue::Text(out)));
                    } else {
                        out.push_str(&old);
                        rest = &rest[idx + old.len()..];
                    }
                }
                out.push_str(rest);
                DaxValue::Text(out)
            } else {
                DaxValue::Text(text.replace(&old, &new))
            }
        }
        "REPLACE" => {
            let text: Vec<char> = arg_str(0).chars().collect();
            let start = (arg_f64(1, 1.0).max(1.0) as usize).saturating_sub(1);
            let num_chars = arg_f64(2, 0.0).max(0.0) as usize;
            let new_text = arg_str(3);
            let before: String = text.iter().take(start).collect();
            let after: String = text.iter().skip(start + num_chars).collect();
            DaxValue::Text(format!("{}{}{}", before, new_text, after))
        }
        "REPT" => {
            let s = arg_str(0);
            let n = arg_f64(1, 0.0).clamp(0.0, 10_000.0) as usize;
            DaxValue::Text(s.repeat(n))
        }
        "EXACT" => DaxValue::Boolean(arg_str(0) == arg_str(1)),
        "VALUE" => {
            let s = arg_str(0);
            let cleaned = s.trim().replace("R$", "").replace('$', "").replace(' ', "");
            let parsed = if cleaned.contains(',') && cleaned.contains('.') {
                cleaned.replace('.', "").replace(',', ".").parse::<f64>()
            } else {
                cleaned.replace(',', ".").parse::<f64>()
            };
            match parsed {
                Ok(n) if n.is_finite() => DaxValue::Number(n),
                _ => DaxValue::Blank,
            }
        }
        "FIXED" => {
            let num = arg_f64(0, 0.0);
            let decimals = arg_f64(1, 2.0).max(0.0) as usize;
            let no_commas = args.get(2).is_some_and(|v| v.as_bool());
            if no_commas {
                DaxValue::Text(format!("{:.prec$}", num, prec = decimals))
            } else {
                DaxValue::Text(format_number_grouped(num, decimals))
            }
        }
        "UNICHAR" => {
            let code = arg_f64(0, 0.0).max(0.0) as u32;
            match char::from_u32(code) {
                Some(ch) => DaxValue::Text(ch.to_string()),
                None => DaxValue::Blank,
            }
        }
        "UNICODE" => {
            match arg_str(0).chars().next() {
                Some(ch) => DaxValue::Number((ch as u32) as f64),
                None => DaxValue::Blank,
            }
        }
        "CONTAINSSTRING" => {
            DaxValue::Boolean(arg_str(0).to_lowercase().contains(&arg_str(1).to_lowercase()))
        }
        "CONTAINSSTRINGEXACT" => DaxValue::Boolean(arg_str(0).contains(&arg_str(1))),
        "COMBINEVALUES" => {
            if args.is_empty() {
                DaxValue::Text(String::new())
            } else {
                let delim = arg_str(0);
                let parts: Vec<String> = args[1..].iter().map(|v| v.as_display_string()).collect();
                DaxValue::Text(parts.join(&delim))
            }
        }
        "FORMAT" => {
            let val = args.first().cloned().unwrap_or(DaxValue::Blank);
            let pat = arg_str(1);
            DaxValue::Text(format_dax_value(&val, &pat))
        }

        // ==========================================
        // Date & Time (Scalar)
        // ==========================================
        "DATE" => {
            let raw_y = arg_f64(0, 1970.0) as i32;
            let y = if (0..100).contains(&raw_y) { 1900 + raw_y } else { raw_y };
            let m = arg_f64(1, 1.0) as i32;
            let d = arg_f64(2, 1.0) as i32;
            let total_m = (y * 12) + (m - 1);
            let norm_y = total_m.div_euclid(12);
            let norm_m = (total_m.rem_euclid(12) + 1) as u32;
            let epoch_days = civil_to_days(norm_y, norm_m, 1) + (d - 1);
            let (fy, fm, fd) = days_to_civil(epoch_days);
            DaxValue::Text(format!("{:04}-{:02}-{:02}", fy, fm, fd))
        }
        "TODAY" | "NOW" => {
            let (y, m, d) = today_ymd();
            DaxValue::Text(format!("{:04}-{:02}-{:02}", y, m, d))
        }
        "DATEVALUE" => {
            if let Some((y, m, d)) = parse_flexible_ymd(&arg_str(0)) {
                DaxValue::Text(format!("{:04}-{:02}-{:02}", y, m, d))
            } else {
                DaxValue::Blank
            }
        }
        "EDATE" => {
            if let Some((y, m, d)) = parse_flexible_ymd(&arg_str(0)) {
                let months = arg_f64(1, 0.0).trunc() as i32;
                let (ny, nm, nd) = add_months(y, m, d, months);
                DaxValue::Text(format!("{:04}-{:02}-{:02}", ny, nm, nd))
            } else {
                DaxValue::Blank
            }
        }
        "EOMONTH" => {
            if let Some((y, m, _)) = parse_flexible_ymd(&arg_str(0)) {
                let months = arg_f64(1, 0.0).trunc() as i32;
                let (ny, nm, _) = add_months(y, m, 1, months);
                let last_d = days_in_month(ny, nm);
                DaxValue::Text(format!("{:04}-{:02}-{:02}", ny, nm, last_d))
            } else {
                DaxValue::Blank
            }
        }
        "WEEKDAY" => {
            if let Some((y, m, d)) = parse_flexible_ymd(&arg_str(0)) {
                let sun0 = day_of_week_0_sun(y, m, d); // 0=Sun..6=Sat
                let ret_type = arg_f64(1, 1.0).trunc() as i32;
                let wd = match ret_type {
                    2 => if sun0 == 0 { 7 } else { sun0 },       // 1=Mon..7=Sun
                    3 => (sun0 + 6) % 7,                          // 0=Mon..6=Sun
                    _ => sun0 + 1,                                // 1=Sun..7=Sat
                };
                DaxValue::Number(wd as f64)
            } else {
                DaxValue::Blank
            }
        }
        "WEEKNUM" => {
            if let Some((y, m, d)) = parse_flexible_ymd(&arg_str(0)) {
                let jan1 = civil_to_days(y, 1, 1);
                let cur = civil_to_days(y, m, d);
                let doy0 = cur - jan1; // 0-indexed day of year
                let ret_type = arg_f64(1, 1.0).trunc() as i32;
                let jan1_dow = day_of_week_0_sun(y, 1, 1) as i32; // 0=Sun
                let start_dow = if ret_type == 2 || ret_type == 21 { 1 } else { 0 };
                let offset = (jan1_dow - start_dow).rem_euclid(7);
                let wnum = (doy0 + offset) / 7 + 1;
                DaxValue::Number(wnum as f64)
            } else {
                DaxValue::Blank
            }
        }
        "QUARTER" => {
            if let Some((_, m, _)) = parse_flexible_ymd(&arg_str(0)) {
                DaxValue::Number((((m - 1) / 3) + 1) as f64)
            } else {
                DaxValue::Blank
            }
        }
        "DATEDIFF" => {
            let d1 = parse_flexible_ymd(&arg_str(0));
            let d2 = parse_flexible_ymd(&arg_str(1));
            let interval = arg_str(2).trim().to_uppercase();
            if let (Some((y1, m1, day1)), Some((y2, m2, day2))) = (d1, d2) {
                let diff_days = civil_to_days(y2, m2, day2) - civil_to_days(y1, m1, day1);
                let val = match interval.as_str() {
                    "YEAR" | "ANO" => (y2 - y1) as f64,
                    "QUARTER" | "TRIMESTRE" => {
                        let q1 = y1 * 4 + ((m1 as i32 - 1) / 3);
                        let q2 = y2 * 4 + ((m2 as i32 - 1) / 3);
                        (q2 - q1) as f64
                    }
                    "MONTH" | "MES" | "MÊS" => ((y2 - y1) * 12 + (m2 as i32 - m1 as i32)) as f64,
                    "WEEK" | "SEMANA" => (diff_days / 7) as f64,
                    _ => diff_days as f64,
                };
                DaxValue::Number(val)
            } else {
                DaxValue::Blank
            }
        }
        "YEARFRAC" => {
            if let (Some((y1, m1, d1)), Some((y2, m2, d2))) =
                (parse_flexible_ymd(&arg_str(0)), parse_flexible_ymd(&arg_str(1)))
            {
                let diff = (civil_to_days(y2, m2, d2) - civil_to_days(y1, m1, d1)).abs() as f64;
                DaxValue::Number(diff / 365.25)
            } else {
                DaxValue::Blank
            }
        }
        "NETWORKDAYS" => {
            if let (Some((y1, m1, d1)), Some((y2, m2, d2))) =
                (parse_flexible_ymd(&arg_str(0)), parse_flexible_ymd(&arg_str(1)))
            {
                let a = civil_to_days(y1, m1, d1);
                let b = civil_to_days(y2, m2, d2);
                let (lo, hi, sign) = if a <= b { (a, b, 1) } else { (b, a, -1) };
                let mut count = 0i32;
                for day in lo..=hi {
                    let dow = (day + 4).rem_euclid(7); // 0=Sun, 6=Sat
                    if dow != 0 && dow != 6 {
                        count += 1;
                    }
                }
                DaxValue::Number((count * sign) as f64)
            } else {
                DaxValue::Blank
            }
        }

        // ==========================================
        // Financial Functions
        // ==========================================
        "PMT" => {
            let rate = arg_f64(0, 0.0);
            let nper = arg_f64(1, 1.0);
            let pv = arg_f64(2, 0.0);
            let fv = arg_f64(3, 0.0);
            let ptype = if arg_f64(4, 0.0) != 0.0 { 1.0 } else { 0.0 };
            if nper.abs() < 1e-12 {
                DaxValue::Blank
            } else if rate.abs() < 1e-12 {
                DaxValue::Number(-(pv + fv) / nper)
            } else {
                let pvif = (1.0 + rate).powf(nper);
                let pmt = -(pv * pvif + fv) * rate / ((1.0 + rate * ptype) * (pvif - 1.0));
                DaxValue::Number(pmt)
            }
        }
        "PV" => {
            let rate = arg_f64(0, 0.0);
            let nper = arg_f64(1, 1.0);
            let pmt = arg_f64(2, 0.0);
            let fv = arg_f64(3, 0.0);
            let ptype = if arg_f64(4, 0.0) != 0.0 { 1.0 } else { 0.0 };
            if rate.abs() < 1e-12 {
                DaxValue::Number(-fv - pmt * nper)
            } else {
                let pvif = (1.0 + rate).powf(nper);
                let pv = (-fv - pmt * (1.0 + rate * ptype) * (pvif - 1.0) / rate) / pvif;
                DaxValue::Number(pv)
            }
        }
        "FV" => {
            let rate = arg_f64(0, 0.0);
            let nper = arg_f64(1, 1.0);
            let pmt = arg_f64(2, 0.0);
            let pv = arg_f64(3, 0.0);
            let ptype = if arg_f64(4, 0.0) != 0.0 { 1.0 } else { 0.0 };
            if rate.abs() < 1e-12 {
                DaxValue::Number(-pv - pmt * nper)
            } else {
                let pvif = (1.0 + rate).powf(nper);
                let fv = -pv * pvif - pmt * (1.0 + rate * ptype) * (pvif - 1.0) / rate;
                DaxValue::Number(fv)
            }
        }
        "NPER" => {
            let rate = arg_f64(0, 0.0);
            let pmt = arg_f64(1, 0.0);
            let pv = arg_f64(2, 0.0);
            let fv = arg_f64(3, 0.0);
            let ptype = if arg_f64(4, 0.0) != 0.0 { 1.0 } else { 0.0 };
            if rate.abs() < 1e-12 {
                if pmt.abs() < 1e-12 {
                    DaxValue::Blank
                } else {
                    DaxValue::Number(-(pv + fv) / pmt)
                }
            } else {
                let z = pmt * (1.0 + rate * ptype) / rate;
                let num = z - fv;
                let den = z + pv;
                if den.abs() < 1e-12 || (num / den) <= 0.0 {
                    DaxValue::Blank
                } else {
                    DaxValue::Number((num / den).ln() / (1.0 + rate).ln())
                }
            }
        }
        "NPV" => {
            let rate = arg_f64(0, 0.0);
            let mut sum = 0.0f64;
            for (idx, v) in args.iter().skip(1).enumerate() {
                if let Some(cf) = v.as_f64() {
                    sum += cf / (1.0 + rate).powi((idx + 1) as i32);
                }
            }
            DaxValue::Number(sum)
        }
        "IPMT" | "PPMT" => {
            let rate = arg_f64(0, 0.0);
            let per = arg_f64(1, 1.0);
            let nper = arg_f64(2, 1.0);
            let pv = arg_f64(3, 0.0);
            let fv = arg_f64(4, 0.0);
            let ptype = if arg_f64(5, 0.0) != 0.0 { 1.0 } else { 0.0 };
            if nper.abs() < 1e-12 || per < 1.0 || per > nper {
                DaxValue::Blank
            } else if rate.abs() < 1e-12 {
                let pmt = -(pv + fv) / nper;
                DaxValue::Number(if fname == "IPMT" { 0.0 } else { pmt })
            } else {
                let pvif = (1.0 + rate).powf(nper);
                let pmt = -(pv * pvif + fv) * rate / ((1.0 + rate * ptype) * (pvif - 1.0));
                let ipmt = if ptype != 0.0 && (per - 1.0).abs() < 1e-12 {
                    0.0
                } else {
                    let effective_per = if ptype != 0.0 { per - 2.0 } else { per - 1.0 };
                    let p_pow = (1.0 + rate).powf(effective_per.max(0.0));
                    let bal = pv * p_pow + pmt * (1.0 + rate * ptype) * (p_pow - 1.0) / rate;
                    if ptype != 0.0 && effective_per < 0.0 {
                        0.0
                    } else {
                        -bal * rate
                    }
                };
                if fname == "IPMT" {
                    DaxValue::Number(ipmt)
                } else {
                    DaxValue::Number(pmt - ipmt)
                }
            }
        }
        "SLN" => {
            let cost = arg_f64(0, 0.0);
            let salvage = arg_f64(1, 0.0);
            let life = arg_f64(2, 1.0);
            if life.abs() < 1e-12 {
                DaxValue::Blank
            } else {
                DaxValue::Number((cost - salvage) / life)
            }
        }
        "SYD" => {
            let cost = arg_f64(0, 0.0);
            let salvage = arg_f64(1, 0.0);
            let life = arg_f64(2, 1.0);
            let per = arg_f64(3, 1.0);
            let sum_years = life * (life + 1.0) / 2.0;
            if sum_years.abs() < 1e-12 {
                DaxValue::Blank
            } else {
                DaxValue::Number((cost - salvage) * (life - per + 1.0) / sum_years)
            }
        }
        "DB" => {
            let cost = arg_f64(0, 0.0);
            let salvage = arg_f64(1, 0.0);
            let life = arg_f64(2, 1.0);
            let period = arg_f64(3, 1.0).max(1.0) as i32;
            let month = arg_f64(4, 12.0).clamp(1.0, 12.0);
            if cost <= 0.0 || life <= 0.0 {
                DaxValue::Number(0.0)
            } else {
                let raw_rate = 1.0 - (salvage / cost).max(0.0).powf(1.0 / life);
                let rate = (raw_rate * 1000.0).round() / 1000.0;
                let mut total_dep = 0.0f64;
                let mut cur_dep = 0.0f64;
                for p in 1..=period {
                    if p == 1 {
                        cur_dep = cost * rate * month / 12.0;
                    } else if (p as f64) > life {
                        cur_dep = ((cost - total_dep) * rate * (12.0 - month)) / 12.0;
                    } else {
                        cur_dep = (cost - total_dep) * rate;
                    }
                    total_dep += cur_dep;
                }
                DaxValue::Number(cur_dep)
            }
        }
        "DDB" => {
            let cost = arg_f64(0, 0.0);
            let salvage = arg_f64(1, 0.0);
            let life = arg_f64(2, 1.0);
            let period = arg_f64(3, 1.0).max(1.0) as i32;
            let factor = arg_f64(4, 2.0);
            if life <= 0.0 {
                DaxValue::Blank
            } else {
                let rate = (factor / life).min(1.0);
                let mut value = cost;
                let mut dep = 0.0f64;
                for _ in 1..=period {
                    dep = (value * rate).min((value - salvage).max(0.0));
                    value -= dep;
                }
                DaxValue::Number(dep.max(0.0))
            }
        }
        "EFFECT" => {
            let nom = arg_f64(0, 0.0);
            let npery = arg_f64(1, 1.0).trunc().max(1.0);
            DaxValue::Number((1.0 + nom / npery).powf(npery) - 1.0)
        }
        "NOMINAL" => {
            let eff = arg_f64(0, 0.0);
            let npery = arg_f64(1, 1.0).trunc().max(1.0);
            DaxValue::Number(npery * ((1.0 + eff).powf(1.0 / npery) - 1.0))
        }
        "RATE" => {
            let nper = arg_f64(0, 1.0);
            let pmt = arg_f64(1, 0.0);
            let pv = arg_f64(2, 0.0);
            let fv = arg_f64(3, 0.0);
            let ptype = if arg_f64(4, 0.0) != 0.0 { 1.0 } else { 0.0 };
            let mut r = arg_f64(5, 0.1);
            if nper <= 0.0 {
                DaxValue::Blank
            } else {
                for _ in 0..50 {
                    if (1.0 + r) <= 0.0 {
                        r = 0.01;
                    }
                    let y = if r.abs() < 1e-10 {
                        pv + pmt * nper + fv
                    } else {
                        let f = (1.0 + r).powf(nper);
                        pv * f + pmt * (1.0 + r * ptype) * (f - 1.0) / r + fv
                    };
                    if y.abs() < 1e-9 {
                        break;
                    }
                    let eps = 1e-6;
                    let r2 = r + eps;
                    let f2 = (1.0 + r2).powf(nper);
                    let y2 = pv * f2 + pmt * (1.0 + r2 * ptype) * (f2 - 1.0) / r2 + fv;
                    let dy = (y2 - y) / eps;
                    if dy.abs() < 1e-12 {
                        break;
                    }
                    r -= y / dy;
                }
                if r.is_finite() { DaxValue::Number(r) } else { DaxValue::Blank }
            }
        }

        // ==========================================
        // Parent-Child Functions
        // ==========================================
        "PATHLENGTH" => {
            let path = arg_str(0);
            if path.trim().is_empty() {
                DaxValue::Number(0.0)
            } else {
                DaxValue::Number(path.split('|').filter(|s| !s.trim().is_empty()).count() as f64)
            }
        }
        "PATHCONTAINS" => {
            let path = arg_str(0);
            let item = arg_str(1);
            let found = path
                .split('|')
                .any(|seg| seg.trim().eq_ignore_ascii_case(item.trim()));
            DaxValue::Boolean(found)
        }
        "PATHITEM" | "PATHITEMREVERSE" => {
            let path = arg_str(0);
            let pos = arg_f64(1, 1.0).trunc() as usize;
            let ret_type = arg_f64(2, 0.0).trunc() as i32;
            let segs: Vec<&str> = path.split('|').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let chosen = if pos == 0 || pos > segs.len() {
                None
            } else if fname == "PATHITEMREVERSE" {
                segs.get(segs.len() - pos).copied()
            } else {
                segs.get(pos - 1).copied()
            };
            match chosen {
                Some(s) if ret_type == 1 => match s.parse::<f64>() {
                    Ok(n) => DaxValue::Number(n),
                    Err(_) => DaxValue::Blank,
                },
                Some(s) => DaxValue::Text(s.to_string()),
                None => DaxValue::Blank,
            }
        }

        _ => return Ok(None),
    };

    Ok(Some(res))
}

/// One dated cash flow for XNPV/XIRR: (amount, day number — any consistent day epoch).
pub type DatedFlow = (f64, i64);

/// Year fractions `(d_i - d_first) / 365` (d_first = earliest date) paired with amounts.
fn xnpv_terms(flows: &[DatedFlow]) -> Vec<(f64, f64)> {
    let first = flows.iter().map(|f| f.1).min().unwrap_or(0);
    flows.iter().map(|&(v, d)| (v, (d - first) as f64 / 365.0)).collect()
}

fn xnpv_at(terms: &[(f64, f64)], rate: f64) -> f64 {
    terms.iter().map(|&(v, t)| v / (1.0 + rate).powf(t)).sum()
}

/// d/d(rate) of `xnpv_at`.
fn xnpv_derivative(terms: &[(f64, f64)], rate: f64) -> f64 {
    terms.iter().map(|&(v, t)| -t * v / (1.0 + rate).powf(t + 1.0)).sum()
}

/// XNPV = Σ v_i / (1 + rate)^((d_i - d_1) / 365), d_1 = earliest date (Microsoft Learn).
pub fn calc_xnpv(rate: f64, flows: &[DatedFlow]) -> Result<f64, String> {
    if rate <= -1.0 {
        return Err(format!("XNPV: taxa deve ser maior que -1 (recebido {})", rate));
    }
    Ok(xnpv_at(&xnpv_terms(flows), rate))
}

const XIRR_TOL: f64 = 1e-10;

/// Newton-Raphson on XNPV(rate) = 0. `None` when it diverges (rate leaves (-1, ∞), non-finite
/// values, flat derivative) or doesn't converge within `max_iter` steps.
fn xirr_newton(terms: &[(f64, f64)], guess: f64, max_iter: usize) -> Option<f64> {
    let scale = terms.iter().map(|t| t.0.abs()).fold(1.0, f64::max);
    let mut r = guess;
    for _ in 0..max_iter {
        if r <= -1.0 || !r.is_finite() {
            return None;
        }
        let f = xnpv_at(terms, r);
        if !f.is_finite() {
            return None;
        }
        if f.abs() <= XIRR_TOL * scale {
            return Some(r);
        }
        let df = xnpv_derivative(terms, r);
        if !df.is_finite() || df.abs() < 1e-300 {
            return None;
        }
        let next = r - f / df;
        if (next - r).abs() <= XIRR_TOL * (1.0 + r.abs()) {
            return (next > -1.0 && xnpv_at(terms, next).abs() <= 1e-6 * scale).then_some(next);
        }
        r = next;
    }
    None
}

/// Bisection fallback: scan a ladder of rates in (-1, ∞) for a sign change of XNPV and
/// bisect it. `None` when no bracket exists (no real IRR in the scanned range).
fn xirr_bisection(terms: &[(f64, f64)]) -> Option<f64> {
    const LADDER: [f64; 18] = [
        -0.999_999, -0.99, -0.9, -0.75, -0.5, -0.25, 0.0, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 100.0,
        1e3, 1e4, 1e6,
    ];
    let scale = terms.iter().map(|t| t.0.abs()).fold(1.0, f64::max);
    let mut prev: Option<(f64, f64)> = None;
    for &r in &LADDER {
        let f = xnpv_at(terms, r);
        if !f.is_finite() {
            prev = None;
            continue;
        }
        if f == 0.0 {
            return Some(r);
        }
        if let Some((pr, pf)) = prev {
            if pf.signum() != f.signum() {
                let (mut lo, mut hi, mut flo) = (pr, r, pf);
                for _ in 0..300 {
                    let mid = 0.5 * (lo + hi);
                    let fm = xnpv_at(terms, mid);
                    if fm.abs() <= XIRR_TOL * scale || (hi - lo).abs() <= XIRR_TOL * (1.0 + mid.abs()) {
                        return Some(mid);
                    }
                    if fm.signum() == flo.signum() {
                        lo = mid;
                        flo = fm;
                    } else {
                        hi = mid;
                    }
                }
                return Some(0.5 * (lo + hi));
            }
        }
        prev = Some((r, f));
    }
    None
}

/// XIRR: the rate r with XNPV(r) = 0. Newton-Raphson from `guess` (DAX default 0.1), falling
/// back to bracketing + bisection when Newton diverges or stalls; a DAX error when neither
/// finds a root (e.g. flows without both signs, or no real IRR).
pub fn calc_xirr(flows: &[DatedFlow], guess: f64) -> Result<f64, String> {
    let has_pos = flows.iter().any(|f| f.0 > 0.0);
    let has_neg = flows.iter().any(|f| f.0 < 0.0);
    if !has_pos || !has_neg {
        return Err("XIRR requer ao menos um fluxo positivo e um negativo".to_string());
    }
    let terms = xnpv_terms(flows);
    xirr_newton(&terms, guess, 100)
        .or_else(|| xirr_bisection(&terms))
        .ok_or_else(|| "XIRR não convergiu: nenhuma taxa zera o XNPV destes fluxos".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fluxo() -> Vec<DatedFlow> {
        // tests/dax-corpus/models/stats.json t_Fluxo
        [((2024, 1, 1), -10000.0), ((2024, 3, 1), 2750.0), ((2024, 6, 1), 4250.0), ((2024, 10, 1), 3250.0), ((2025, 1, 1), 2750.0)]
            .iter()
            .map(|&((y, m, d), v)| (v, civil_to_days(y, m, d) as i64))
            .collect()
    }

    #[test]
    fn test_xnpv_formula_and_rate_domain() {
        let flows = fluxo();
        assert!((calc_xnpv(0.10, &flows).unwrap() - 2316.808365460487).abs() < 1e-6);
        // Rate 0 is the plain sum; order of the rows doesn't matter (d_1 = earliest date).
        assert!((calc_xnpv(0.0, &flows).unwrap() - 3000.0).abs() < 1e-9);
        let mut rev = flows.clone();
        rev.reverse();
        assert!((calc_xnpv(0.10, &rev).unwrap() - 2316.808365460487).abs() < 1e-6);
        assert!(calc_xnpv(-1.0, &flows).is_err());
    }

    #[test]
    fn test_xirr_newton_bisection_fallback_and_failure() {
        let flows = fluxo();
        let expected = 0.6122389541287326;
        // Newton from the default guess converges on its own.
        let terms = xnpv_terms(&flows);
        let newton = xirr_newton(&terms, 0.1, 100).expect("newton converges from 0.1");
        assert!((newton - expected).abs() < 1e-8);
        assert!((calc_xirr(&flows, 0.1).unwrap() - expected).abs() < 1e-8);
        // Bisection alone finds the same root.
        assert!((xirr_bisection(&terms).unwrap() - expected).abs() < 1e-8);

        // -100 now, +110 in one year: IRR = 10%. From guess 50 the first Newton step jumps
        // below -1 (diverges), so the bisection fallback must provide the answer.
        let simple = vec![(-100.0, 0), (110.0, 365)];
        assert!(xirr_newton(&xnpv_terms(&simple), 50.0, 100).is_none());
        assert!((calc_xirr(&simple, 50.0).unwrap() - 0.10).abs() < 1e-8);

        // Mixed signs but no real root: 100 - 300x + 250x² > 0 for every x = 1/(1+r).
        let no_root = vec![(100.0, 0), (-300.0, 365), (250.0, 730)];
        assert!(calc_xirr(&no_root, 0.1).is_err());
        // Single-signed flows have no IRR.
        assert!(calc_xirr(&[(100.0, 0), (50.0, 365)], 0.1).is_err());
    }


    #[test]
    fn test_scalar_math_text_date_financial_functions() {
        assert_eq!(
            try_eval_scalar_fn("ROUNDUP", &[DaxValue::Number(3.141), DaxValue::Number(2.0)]).unwrap(),
            Some(DaxValue::Number(3.15))
        );
        assert_eq!(
            try_eval_scalar_fn("MROUND", &[DaxValue::Number(17.0), DaxValue::Number(5.0)]).unwrap(),
            Some(DaxValue::Number(15.0))
        );
        assert_eq!(
            try_eval_scalar_fn("GCD", &[DaxValue::Number(48.0), DaxValue::Number(18.0)]).unwrap(),
            Some(DaxValue::Number(6.0))
        );
        assert_eq!(
            try_eval_scalar_fn("MID", &[DaxValue::Text("PrismBI".into()), DaxValue::Number(6.0), DaxValue::Number(2.0)]).unwrap(),
            Some(DaxValue::Text("BI".into()))
        );
        assert_eq!(
            try_eval_scalar_fn("CONTAINSSTRING", &[DaxValue::Text("Power BI Linux".into()), DaxValue::Text("linux".into())]).unwrap(),
            Some(DaxValue::Boolean(true))
        );
        assert_eq!(
            try_eval_scalar_fn("FORMAT", &[DaxValue::Number(1234.5), DaxValue::Text("R$ #,##0.00".into())]).unwrap(),
            Some(DaxValue::Text("R$ 1,234.50".into()))
        );
        assert_eq!(
            try_eval_scalar_fn("DATEDIFF", &[DaxValue::Text("2026-01-15".into()), DaxValue::Text("2026-04-20".into()), DaxValue::Text("MONTH".into())]).unwrap(),
            Some(DaxValue::Number(3.0))
        );
        assert_eq!(
            try_eval_scalar_fn("EOMONTH", &[DaxValue::Text("2024-02-10".into()), DaxValue::Number(0.0)]).unwrap(),
            Some(DaxValue::Text("2024-02-29".into()))
        );
        assert_eq!(
            try_eval_scalar_fn("NETWORKDAYS", &[DaxValue::Text("2026-11-23".into()), DaxValue::Text("2026-11-29".into())]).unwrap(),
            Some(DaxValue::Number(5.0))
        );
        let pmt = try_eval_scalar_fn("PMT", &[DaxValue::Number(0.01), DaxValue::Number(12.0), DaxValue::Number(10000.0)])
            .unwrap()
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((pmt - (-888.4878867)).abs() < 1e-4);

        // v0.7.0 expanded scalar coverage tests
        assert_eq!(
            try_eval_scalar_fn("ISBOOLEAN", &[DaxValue::Boolean(true)]).unwrap(),
            Some(DaxValue::Boolean(true))
        );
        assert_eq!(
            try_eval_scalar_fn("ISINT64", &[DaxValue::Number(42.0)]).unwrap(),
            Some(DaxValue::Boolean(true))
        );
        assert_eq!(
            try_eval_scalar_fn("COALESCE", &[DaxValue::Blank, DaxValue::Text("ok".into())]).unwrap(),
            Some(DaxValue::Text("ok".into()))
        );
        assert_eq!(
            try_eval_scalar_fn("COMBIN", &[DaxValue::Number(5.0), DaxValue::Number(2.0)]).unwrap(),
            Some(DaxValue::Number(10.0))
        );
        assert_eq!(
            try_eval_scalar_fn("BITLSHIFT", &[DaxValue::Number(3.0), DaxValue::Number(2.0)]).unwrap(),
            Some(DaxValue::Number(12.0))
        );
        assert_eq!(
            try_eval_scalar_fn("PATHCONTAINS", &[DaxValue::Text("A|B|C".into()), DaxValue::Text("B".into())]).unwrap(),
            Some(DaxValue::Boolean(true))
        );
        assert_eq!(
            try_eval_scalar_fn("PATHITEM", &[DaxValue::Text("10|20|30".into()), DaxValue::Number(2.0), DaxValue::Number(1.0)]).unwrap(),
            Some(DaxValue::Number(20.0))
        );
        assert_eq!(
            try_eval_scalar_fn("SLN", &[DaxValue::Number(10000.0), DaxValue::Number(1000.0), DaxValue::Number(9.0)]).unwrap(),
            Some(DaxValue::Number(1000.0))
        );
        let eff = try_eval_scalar_fn("EFFECT", &[DaxValue::Number(0.12), DaxValue::Number(12.0)]).unwrap().unwrap().as_f64().unwrap();
        assert!((eff - 0.126825).abs() < 1e-4);
    }
}
