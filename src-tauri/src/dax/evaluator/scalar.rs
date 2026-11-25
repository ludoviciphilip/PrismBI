//! Logical, math, text and date-part functions whose arguments are evaluated lazily.

use super::*;
use crate::dax::functions::parse_iso_ymd;

/// DAX `ISBLANK`.
pub(super) fn eval_isblank(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let v = eval_arg(args, 0, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Boolean(v.is_blank()))
}

/// DAX `NOT`.
pub(super) fn eval_not(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let v = eval_arg(args, 0, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Boolean(!v.as_bool()))
}

/// DAX `AND`.
pub(super) fn eval_and(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let a = eval_arg(args, 0, model, ctx, row_stack, vars)?;
    let b = eval_arg(args, 1, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Boolean(a.as_bool() && b.as_bool()))
}

/// DAX `OR`.
pub(super) fn eval_or(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let a = eval_arg(args, 0, model, ctx, row_stack, vars)?;
    let b = eval_arg(args, 1, model, ctx, row_stack, vars)?;
    Ok(DaxValue::Boolean(a.as_bool() || b.as_bool()))
}

/// DAX `IF`, `IF.EAGER`.
pub(super) fn eval_if(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let cond = eval_arg(args, 0, model, ctx, row_stack, vars)?;
    if cond.as_bool() {
        eval_arg(args, 1, model, ctx, row_stack, vars)
    } else if args.len() >= 3 {
        eval_arg(args, 2, model, ctx, row_stack, vars)
    } else {
        Ok(DaxValue::Blank)
    }
}

/// DAX `SWITCH`.
pub(super) fn eval_switch(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    if args.len() < 3 {
        return Ok(DaxValue::Blank);
    }
    let target = evaluate_ast(&args[0], model, ctx, row_stack, vars)?;
    let mut idx = 1;
    while idx + 1 < args.len() {
        let cand = evaluate_ast(&args[idx], model, ctx, row_stack, vars)?;
        if target.dax_equals(&cand) {
            return evaluate_ast(&args[idx + 1], model, ctx, row_stack, vars);
        }
        idx += 2;
    }
    if idx < args.len() {
        evaluate_ast(&args[idx], model, ctx, row_stack, vars)
    } else {
        Ok(DaxValue::Blank)
    }
}

/// DAX `DIVIDE`.
pub(super) fn eval_divide(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let num = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    let den = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    if den.abs() < 1e-12 {
        if args.len() >= 3 {
            eval_arg(args, 2, model, ctx, row_stack, vars)
        } else {
            Ok(DaxValue::Blank)
        }
    } else {
        Ok(DaxValue::number_smart(num / den))
    }
}

/// DAX `COALESCE`.
pub(super) fn eval_coalesce(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    for a in args {
        let v = evaluate_ast(a, model, ctx, row_stack, vars)?;
        if !v.is_blank() {
            return Ok(v);
        }
    }
    Ok(DaxValue::Blank)
}

/// DAX `IFERROR`.
pub(super) fn eval_iferror(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    match eval_arg(args, 0, model, ctx, row_stack, vars) {
    Ok(v) if v.as_f64().is_some_and(|n| !n.is_finite()) => {
        eval_arg(args, 1, model, ctx, row_stack, vars)
    }
    Ok(v) => Ok(v),
    Err(_) => eval_arg(args, 1, model, ctx, row_stack, vars),
}
}

/// DAX `ISERROR`.
pub(super) fn eval_iserror(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    match evaluate_ast(&args[0], model, ctx, row_stack, vars) {
    Ok(v) if v.as_f64().is_some_and(|n| !n.is_finite()) => Ok(DaxValue::Boolean(true)),
    Ok(_) => Ok(DaxValue::Boolean(false)),
    Err(_) => Ok(DaxValue::Boolean(true)),
}
}

/// DAX `ABS`.
pub(super) fn eval_abs(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    Ok(DaxValue::number_smart(
    eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0).abs(),
))
}

/// DAX `ROUND`.
pub(super) fn eval_round(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    let digits = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0) as i32;
    let factor = 10f64.powi(digits);
    Ok(DaxValue::number_smart((n * factor).round() / factor))
}

/// DAX `INT`.
pub(super) fn eval_int(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    Ok(DaxValue::number_smart(n.floor()))
}

/// DAX `SQRT`.
pub(super) fn eval_sqrt(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let n = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    if n < 0.0 {
        Err("SQRT: número negativo".to_string())
    } else {
        Ok(DaxValue::number_smart(n.sqrt()))
    }
}

/// DAX `POWER`.
pub(super) fn eval_power(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let base = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    let exp = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(0.0);
    Ok(DaxValue::number_smart(base.powf(exp)))
}

/// DAX `UPPER`.
pub(super) fn eval_upper(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    Ok(DaxValue::Text(
    eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().to_uppercase(),
))
}

/// DAX `LOWER`.
pub(super) fn eval_lower(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    Ok(DaxValue::Text(
    eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().to_lowercase(),
))
}

/// DAX `LEN`.
pub(super) fn eval_len(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    Ok(DaxValue::number_smart(
    eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string().chars().count() as f64,
))
}

/// DAX `CONCATENATE`.
pub(super) fn eval_concatenate(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let a = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
    let b = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_display_string();
    Ok(DaxValue::Text(format!("{}{}", a, b)))
}

/// DAX `LEFT`.
pub(super) fn eval_left(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
    let n = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(1.0).max(0.0) as usize;
    Ok(DaxValue::Text(s.chars().take(n).collect()))
}

/// DAX `RIGHT`.
pub(super) fn eval_right(
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
    let n = eval_arg(args, 1, model, ctx, row_stack, vars)?.as_f64().unwrap_or(1.0).max(0.0) as usize;
    let chars: Vec<char> = s.chars().collect();
    let start = chars.len().saturating_sub(n);
    Ok(DaxValue::Text(chars[start..].iter().collect()))
}

/// DAX `YEAR`, `MONTH`, `DAY`.
pub(super) fn eval_date_part(
    fname: &str,
    args: &[Expr],
    model: &DaxModel,
    ctx: &FilterContext,
    row_stack: &[RowContext<'_>],
    vars: &HashMap<String, DaxValue>,
) -> Result<DaxValue, String> {
    let s = eval_arg(args, 0, model, ctx, row_stack, vars)?.as_display_string();
    if let Some((y, m, d)) = parse_iso_ymd(&s) {
        let val = match fname {
            "YEAR" => y as f64,
            "MONTH" => m as f64,
            _ => d as f64,
        };
        Ok(DaxValue::number_smart(val))
    } else {
        Ok(DaxValue::Blank)
    }
}
