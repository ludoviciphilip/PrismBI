//! M AST → `EtlQuery`.
//!
//! Each `let` binding becomes one step when it matches a known Power Query
//! function applied to the previous binding. Anything else becomes an
//! `opaqueM` step that keeps the original M text (plus a warning), so the
//! round-trip never loses user code. Unknown sources become `opaqueM` sources.

use super::m_parse::{parse_document, BinOp, Binding, Expr, ExprKind, TypeExpr, UnOp};
use super::m_to::canonical_source_binding;
use super::model::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct FromMResult {
    pub query: EtlQuery,
    pub warnings: Vec<String>,
}

/// Parses `m` and converts it to an `EtlQuery` named `name`.
pub fn query_from_m(name: &str, m: &str) -> Result<FromMResult, String> {
    let doc = parse_document(m).map_err(|e| format!("Erro de sintaxe M: {}", e))?;
    let mut warnings = Vec::new();

    let (mut bindings, body): (Vec<Binding>, Expr) = match doc.kind {
        ExprKind::Let { bindings, body } => (bindings, *body),
        _ => {
            let span = doc.span;
            (
                vec![Binding {
                    name: "Source".into(),
                    expr: doc,
                }],
                Expr {
                    kind: ExprKind::Ident("Source".into()),
                    span,
                },
            )
        }
    };
    if bindings.is_empty() {
        return Err("Script M sem etapas".into());
    }

    // Output binding: normally the last one.
    match body.unparen().as_ident() {
        Some(out) => match bindings.iter().position(|b| b.name == out) {
            Some(k) => {
                if k + 1 < bindings.len() {
                    warnings.push(format!(
                        "Etapas após '{}' não são usadas pelo resultado e foram descartadas.",
                        out
                    ));
                    bindings.truncate(k + 1);
                }
            }
            None => {
                // `in` names something outside the let (another query).
                bindings.push(Binding {
                    name: "Resultado".into(),
                    expr: body.clone(),
                });
            }
        },
        None => bindings.push(Binding {
            name: "Resultado".into(),
            expr: body.clone(),
        }),
    }

    let names: HashSet<String> = bindings.iter().map(|b| b.name.clone()).collect();
    let conv = Converter { src: m, names: &names };

    let src_match = conv.recognize_source(&bindings, &mut warnings);
    let canonical = canonical_source_binding(&src_match.source);
    let mut steps: Vec<EtlStep> = Vec::new();
    let mut prev = src_match.final_name.clone();
    let mut i = src_match.consumed;
    let mut id_seq = 0usize;
    let mut next_id = || {
        id_seq += 1;
        format!("step-{}", id_seq)
    };

    while i < bindings.len() {
        let b = &bindings[i];
        // NestedJoin followed by ExpandTableColumn on it → one mergeQueries step.
        if let Some(next) = bindings.get(i + 1) {
            if let Some(kind) = conv.merge_two_bindings(&b.expr, &b.name, &next.expr, &prev) {
                steps.push(EtlStep::new(next_id(), next.name.clone(), kind));
                prev = next.name.clone();
                i += 2;
                continue;
            }
        }
        match conv.convert_step(&b.name, &b.expr, &prev) {
            Some(new_steps) => {
                for (n, k) in new_steps {
                    steps.push(EtlStep::new(next_id(), n, k));
                }
            }
            None => {
                let mut text = b.expr.text(m).to_string();
                if src_match.final_name != canonical {
                    text = rename_ident_refs(&b.expr, m, &src_match.final_name, &canonical);
                }
                let fname = describe_head(&b.expr);
                warnings.push(format!(
                    "Etapa '{}' ({}) não é suportada pelo editor visual; mantida como M opaco.",
                    b.name, fname
                ));
                steps.push(EtlStep::new(next_id(), b.name.clone(), StepKind::OpaqueM { m: text }));
            }
        }
        prev = b.name.clone();
        i += 1;
    }

    Ok(FromMResult {
        query: EtlQuery {
            name: name.to_string(),
            source: src_match.source,
            steps,
        },
        warnings,
    })
}

fn describe_head(e: &Expr) -> String {
    match e.as_call() {
        Some((f, _)) => f.to_string(),
        None => match &e.kind {
            ExprKind::Opaque(_) => "M não reconhecido".into(),
            _ => "expressão".into(),
        },
    }
}

/// Re-emits the text of `e` replacing identifier references `old` by `new`.
fn rename_ident_refs(e: &Expr, src: &str, old: &str, new: &str) -> String {
    let mut spans = Vec::new();
    collect_ident_spans(e, old, &mut spans);
    let (s0, s1) = e.span;
    let mut text = src[s0..s1].to_string();
    spans.sort_by(|a, b| b.0.cmp(&a.0));
    for (a, b) in spans {
        text.replace_range(a - s0..b - s0, &super::m_parse::quote_m_ident(new));
    }
    text.trim().to_string()
}

fn collect_ident_spans(e: &Expr, name: &str, out: &mut Vec<(usize, usize)>) {
    match &e.kind {
        ExprKind::Ident(n) if n == name => out.push(e.span),
        ExprKind::Let { bindings, body } => {
            for b in bindings {
                collect_ident_spans(&b.expr, name, out);
            }
            collect_ident_spans(body, name, out);
        }
        ExprKind::Each(x) | ExprKind::Paren(x) | ExprKind::Error(x) => collect_ident_spans(x, name, out),
        ExprKind::Func { body, .. } => collect_ident_spans(body, name, out),
        ExprKind::If { cond, then, els } => {
            collect_ident_spans(cond, name, out);
            collect_ident_spans(then, name, out);
            collect_ident_spans(els, name, out);
        }
        ExprKind::Record(fields) => fields.iter().for_each(|(_, v)| collect_ident_spans(v, name, out)),
        ExprKind::List(items) => items.iter().for_each(|v| collect_ident_spans(v, name, out)),
        ExprKind::Field { target: Some(t), .. } => collect_ident_spans(t, name, out),
        ExprKind::Item { target, key, .. } => {
            collect_ident_spans(target, name, out);
            collect_ident_spans(key, name, out);
        }
        ExprKind::Call { func, args } => {
            collect_ident_spans(func, name, out);
            args.iter().for_each(|a| collect_ident_spans(a, name, out));
        }
        ExprKind::Unary { expr, .. } => collect_ident_spans(expr, name, out),
        ExprKind::Binary { left, right, .. } => {
            collect_ident_spans(left, name, out);
            collect_ident_spans(right, name, out);
        }
        ExprKind::Try { expr, otherwise } => {
            collect_ident_spans(expr, name, out);
            if let Some(o) = otherwise {
                collect_ident_spans(o, name, out);
            }
        }
        ExprKind::Range(a, b) => {
            collect_ident_spans(a, name, out);
            collect_ident_spans(b, name, out);
        }
        _ => {}
    }
}

struct SourceMatch {
    source: EtlSource,
    consumed: usize,
    final_name: String,
}

struct Converter<'a> {
    src: &'a str,
    names: &'a HashSet<String>,
}

// ------------------------------------------------------------ small helpers

pub(crate) fn scalar_of(e: &Expr) -> Option<Scalar> {
    let e = e.unparen();
    match &e.kind {
        ExprKind::Null => Some(Scalar::Null),
        ExprKind::Bool(b) => Some(Scalar::Bool(*b)),
        ExprKind::Text(t) => Some(Scalar::Text(t.clone())),
        _ => e.as_number().filter(|n| n.is_finite()).map(Scalar::Number),
    }
}

fn text_of(e: &Expr) -> Option<String> {
    e.unparen().as_text().map(str::to_string)
}

/// `{"a", "b"}` or a single `"a"`.
fn text_list(e: &Expr) -> Option<Vec<String>> {
    match &e.unparen().kind {
        ExprKind::List(items) => items.iter().map(text_of).collect(),
        ExprKind::Text(t) => Some(vec![t.clone()]),
        _ => None,
    }
}

fn u64_of(e: &Expr) -> Option<u64> {
    let n = e.as_number()?;
    (n >= 0.0 && n.fract() == 0.0 && n < 9.0e15).then_some(n as u64)
}

fn i64_of(e: &Expr) -> Option<i64> {
    let n = e.as_number()?;
    (n.fract() == 0.0 && n.abs() < 9.0e15).then_some(n as i64)
}

/// `[Col]` (implicit `_`).
fn field_of(e: &Expr) -> Option<String> {
    match &e.unparen().kind {
        ExprKind::Field {
            target: None,
            name,
            optional: false,
        } => Some(name.clone()),
        _ => None,
    }
}

/// M type expression → EtlType. `Ok(None)` = `type any`.
pub(crate) fn etl_type_of(e: &Expr) -> Result<Option<EtlType>, ()> {
    match &e.unparen().kind {
        ExprKind::Type(TypeExpr::Primitive { name, .. }) => match name.as_str() {
            "text" => Ok(Some(EtlType::Text)),
            "number" => Ok(Some(EtlType::Decimal)),
            "date" => Ok(Some(EtlType::Date)),
            "datetime" => Ok(Some(EtlType::Datetime)),
            "logical" => Ok(Some(EtlType::Boolean)),
            "any" | "anynonnull" => Ok(None),
            _ => Err(()),
        },
        ExprKind::Ident(name) => match name.as_str() {
            "Int64.Type" | "Int32.Type" | "Int16.Type" | "Int8.Type" | "Byte.Type" => Ok(Some(EtlType::Integer)),
            "Currency.Type" => Ok(Some(EtlType::Currency)),
            "Number.Type" | "Decimal.Type" | "Double.Type" | "Single.Type" | "Percentage.Type" => {
                Ok(Some(EtlType::Decimal))
            }
            "Text.Type" => Ok(Some(EtlType::Text)),
            "Date.Type" => Ok(Some(EtlType::Date)),
            "DateTime.Type" => Ok(Some(EtlType::Datetime)),
            "Logical.Type" => Ok(Some(EtlType::Boolean)),
            "Any.Type" => Ok(None),
            _ => Err(()),
        },
        _ => Err(()),
    }
}

fn file_contents_path(e: &Expr) -> Option<String> {
    let (f, args) = e.unparen().as_call()?;
    (f == "File.Contents" && !args.is_empty()).then(|| text_of(&args[0]))?
}

// ------------------------------------------------------------ conditions

/// Comparison operand order: `[c] op v` or `v op [c]` (flipped).
fn compare_cond(op: BinOp, left: &Expr, right: &Expr) -> Option<Cond> {
    let (col, val, flipped) = if let Some(c) = field_of(left) {
        (c, scalar_of(right)?, false)
    } else if let Some(c) = field_of(right) {
        (c, scalar_of(left)?, true)
    } else {
        return None;
    };
    let op = match (op, flipped) {
        (BinOp::Eq, _) => {
            if val.is_null() {
                CondOp::IsNull
            } else {
                CondOp::Eq { value: val }
            }
        }
        (BinOp::Ne, _) => {
            if val.is_null() {
                CondOp::IsNotNull
            } else {
                CondOp::Neq { value: val }
            }
        }
        (BinOp::Gt, false) | (BinOp::Lt, true) => CondOp::Gt { value: val },
        (BinOp::Ge, false) | (BinOp::Le, true) => CondOp::Gte { value: val },
        (BinOp::Lt, false) | (BinOp::Gt, true) => CondOp::Lt { value: val },
        (BinOp::Le, false) | (BinOp::Ge, true) => CondOp::Lte { value: val },
        _ => return None,
    };
    Some(Cond::cmp(col, op))
}

fn flatten_bool<'e>(e: &'e Expr, op: BinOp, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Binary { op: o, left, right } if *o == op => {
            flatten_bool(left, op, out);
            flatten_bool(right, op, out);
        }
        _ => out.push(e),
    }
}

fn leaf_parts(c: &Cond) -> Option<(&str, &CondOp)> {
    match c {
        Cond::Compare(CompareCond { column, op }) => Some((column.as_str(), op)),
        _ => None,
    }
}

/// `each` body → `Cond`.
pub(crate) fn cond_of(e: &Expr) -> Option<Cond> {
    match &e.kind {
        ExprKind::Paren(inner) => cond_of(inner),
        ExprKind::Binary {
            op: op @ (BinOp::And | BinOp::Or),
            ..
        } => {
            let mut parts = Vec::new();
            flatten_bool(e, *op, &mut parts);
            let conds: Vec<Cond> = parts.into_iter().map(cond_of).collect::<Option<_>>()?;
            if conds.len() == 2 {
                if let (Some((c1, o1)), Some((c2, o2))) = (leaf_parts(&conds[0]), leaf_parts(&conds[1])) {
                    let empty = Scalar::text("");
                    if c1 == c2 {
                        match (op, o1, o2) {
                            (BinOp::Or, CondOp::IsNull, CondOp::Eq { value }) if *value == empty => {
                                return Some(Cond::cmp(c1, CondOp::IsBlank))
                            }
                            (BinOp::And, CondOp::IsNotNull, CondOp::Neq { value }) if *value == empty => {
                                return Some(Cond::cmp(c1, CondOp::IsNotBlank))
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some(if *op == BinOp::And {
                Cond::And { and: conds }
            } else {
                Cond::Or { or: conds }
            })
        }
        ExprKind::Unary { op: UnOp::Not, expr } => {
            if let ExprKind::Call { .. } = expr.kind {
                match cond_of(expr)? {
                    Cond::Compare(CompareCond {
                        column,
                        op: CondOp::Contains { value },
                    }) => return Some(Cond::cmp(column, CondOp::NotContains { value })),
                    Cond::Compare(CompareCond {
                        column,
                        op: CondOp::In { values },
                    }) => return Some(Cond::cmp(column, CondOp::NotIn { values })),
                    other => return Some(Cond::Not { not: Box::new(other) }),
                }
            }
            Some(Cond::Not {
                not: Box::new(cond_of(expr)?),
            })
        }
        ExprKind::Binary { op, left, right } => compare_cond(*op, left, right),
        ExprKind::Call { .. } => {
            let (f, args) = e.as_call()?;
            match (f, args) {
                ("Text.Contains" | "Text.StartsWith" | "Text.EndsWith", [a, b]) => {
                    let column = field_of(a)?;
                    let value = scalar_of(b)?;
                    let op = match f {
                        "Text.Contains" => CondOp::Contains { value },
                        "Text.StartsWith" => CondOp::StartsWith { value },
                        _ => CondOp::EndsWith { value },
                    };
                    Some(Cond::cmp(column, op))
                }
                ("List.Contains", [list, item]) => {
                    let column = field_of(item)?;
                    let values = list
                        .unparen()
                        .as_list()?
                        .iter()
                        .map(scalar_of)
                        .collect::<Option<Vec<_>>>()?;
                    Some(Cond::cmp(column, CondOp::In { values }))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_remove_blank_rows(body: &Expr) -> bool {
    // not List.IsEmpty(List.RemoveMatchingItems(Record.FieldValues(_), {"", null}))
    let ExprKind::Unary { op: UnOp::Not, expr } = &body.unparen().kind else {
        return false;
    };
    let Some(("List.IsEmpty", [inner])) = expr.unparen().as_call() else {
        return false;
    };
    let Some(("List.RemoveMatchingItems", [values, matches])) = inner.unparen().as_call() else {
        return false;
    };
    let Some(("Record.FieldValues", [u])) = values.unparen().as_call() else {
        return false;
    };
    if u.unparen().as_ident() != Some("_") {
        return false;
    }
    let Some(items) = matches.unparen().as_list() else {
        return false;
    };
    let scalars: Option<Vec<Scalar>> = items.iter().map(scalar_of).collect();
    let Some(scalars) = scalars else { return false };
    scalars.len() == 2 && scalars.contains(&Scalar::Null) && scalars.contains(&Scalar::text(""))
}

fn value_or_column(e: &Expr) -> Option<ValueOrColumn> {
    if let Some(c) = field_of(e) {
        return Some(ValueOrColumn::Column { column: c });
    }
    scalar_of(e).map(ValueOrColumn::Value)
}

/// `if c1 then v1 else if c2 then v2 else v3` with Cond-shaped conditions and
/// literal/column branches.
fn conditional_of(body: &Expr) -> Option<(Vec<ConditionalRule>, ValueOrColumn)> {
    let mut rules = Vec::new();
    let mut cur = body;
    loop {
        match &cur.kind {
            ExprKind::If { cond, then, els } => {
                rules.push(ConditionalRule {
                    when: cond_of(cond)?,
                    then: value_or_column(then)?,
                });
                cur = els;
            }
            _ => {
                if rules.is_empty() {
                    return None;
                }
                return Some((rules, value_or_column(cur)?));
            }
        }
    }
}

fn agg_list_fn(e: &Expr) -> Option<AggOp> {
    if let ExprKind::Each(body) = &e.unparen().kind {
        // each List.Count(List.Distinct(_))
        if let Some(("List.Count", [inner])) = body.unparen().as_call() {
            if let Some(("List.Distinct", [u])) = inner.unparen().as_call() {
                if u.unparen().as_ident() == Some("_") {
                    return Some(AggOp::CountDistinct);
                }
            }
        }
        return None;
    }
    Some(match e.unparen().as_ident()? {
        "List.Sum" => AggOp::Sum,
        "List.Average" => AggOp::Avg,
        "List.Min" => AggOp::Min,
        "List.Max" => AggOp::Max,
        "List.NonNullCount" => AggOp::Count,
        "List.Count" => AggOp::CountRows,
        "List.Median" => AggOp::Median,
        _ => return None,
    })
}

fn group_agg(e: &Expr) -> Option<Agg> {
    let items = e.unparen().as_list()?;
    if items.len() < 2 || items.len() > 3 {
        return None;
    }
    let name = text_of(&items[0])?;
    let ExprKind::Each(body) = &items[1].unparen().kind else {
        return None;
    };
    let (f, args) = body.unparen().as_call()?;
    let (op, column) = match (f, args) {
        ("List.Sum", [c]) => (AggOp::Sum, Some(field_of(c)?)),
        ("List.Average", [c]) => (AggOp::Avg, Some(field_of(c)?)),
        ("List.Min", [c]) => (AggOp::Min, Some(field_of(c)?)),
        ("List.Max", [c]) => (AggOp::Max, Some(field_of(c)?)),
        ("List.Median", [c]) => (AggOp::Median, Some(field_of(c)?)),
        ("List.NonNullCount", [c]) => (AggOp::Count, Some(field_of(c)?)),
        ("List.Count", [c]) => {
            if let Some(("List.Distinct", [inner])) = c.unparen().as_call() {
                (AggOp::CountDistinct, Some(field_of(inner)?))
            } else {
                field_of(c)?;
                (AggOp::CountRows, None)
            }
        }
        ("Table.RowCount", [t]) => {
            if t.unparen().as_ident() == Some("_") {
                (AggOp::CountRows, None)
            } else if let Some(("Table.Distinct", [u])) = t.unparen().as_call() {
                if u.unparen().as_ident() != Some("_") {
                    return None;
                }
                (AggOp::CountDistinct, None)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(Agg { name, op, column })
}

fn join_kind_of(e: &Expr) -> Option<JoinKind> {
    Some(match e.unparen().as_ident()? {
        "JoinKind.LeftOuter" => JoinKind::LeftOuter,
        "JoinKind.Inner" => JoinKind::Inner,
        "JoinKind.RightOuter" => JoinKind::RightOuter,
        "JoinKind.FullOuter" => JoinKind::FullOuter,
        "JoinKind.LeftAnti" => JoinKind::LeftAnti,
        "JoinKind.RightAnti" => JoinKind::RightAnti,
        _ => return None,
    })
}

fn text_fn_of(e: &Expr) -> Option<TextFn> {
    let name = match &e.unparen().kind {
        ExprKind::Ident(n) => n.clone(),
        // each Text.Upper(_)
        ExprKind::Each(body) => {
            let (f, args) = body.unparen().as_call()?;
            if args.len() != 1 || args[0].unparen().as_ident() != Some("_") {
                return None;
            }
            f.to_string()
        }
        _ => return None,
    };
    Some(match name.as_str() {
        "Text.Upper" => TextFn::Upper,
        "Text.Lower" => TextFn::Lower,
        "Text.Proper" => TextFn::Proper,
        "Text.Trim" => TextFn::Trim,
        "Text.Clean" => TextFn::Clean,
        _ => return None,
    })
}

fn quote_style_ok(e: Option<&Expr>) -> bool {
    match e {
        None => true,
        Some(e) => matches!(e.unparen().as_ident(), Some("QuoteStyle.Csv" | "QuoteStyle.None")) || e.is_null(),
    }
}

/// Splitter.* → (mode, delimiter, positions, length).
#[allow(clippy::type_complexity)]
fn splitter_of(e: &Expr) -> Option<(SplitMode, Option<String>, Option<Vec<u64>>, Option<u64>)> {
    let (f, args) = e.unparen().as_call()?;
    match f {
        "Splitter.SplitTextByDelimiter" => {
            if args.is_empty() || args.len() > 2 || !quote_style_ok(args.get(1)) {
                return None;
            }
            Some((SplitMode::DelimiterEach, Some(text_of(&args[0])?), None, None))
        }
        "Splitter.SplitTextByEachDelimiter" => {
            if args.is_empty() || args.len() > 3 || !quote_style_ok(args.get(1)) {
                return None;
            }
            let delims = text_list(&args[0])?;
            if delims.len() != 1 {
                return None;
            }
            let from_end = match args.get(2) {
                None => false,
                Some(b) => match b.unparen().kind {
                    ExprKind::Bool(v) => v,
                    _ => return None,
                },
            };
            let mode = if from_end {
                SplitMode::DelimiterRightmost
            } else {
                SplitMode::DelimiterLeftmost
            };
            Some((mode, Some(delims[0].clone()), None, None))
        }
        "Splitter.SplitTextByPositions" => {
            if args.len() != 1 {
                return None;
            }
            let pos = args[0]
                .unparen()
                .as_list()?
                .iter()
                .map(u64_of)
                .collect::<Option<Vec<_>>>()?;
            Some((SplitMode::Positions, None, Some(pos), None))
        }
        "Splitter.SplitTextByRepeatedLengths" => {
            if args.len() != 1 {
                return None;
            }
            Some((SplitMode::FixedLength, None, None, Some(u64_of(&args[0])?)))
        }
        _ => None,
    }
}

/// `{{"c", type}, …}` with optional culture → changeType parts.
fn change_type_specs(e: &Expr) -> Option<Vec<ColumnTypeSpec>> {
    let items = e.unparen().as_list()?;
    // Single pair `{"c", type text}`
    if items.len() == 2 && text_of(&items[0]).is_some() && items[1].unparen().as_list().is_none() {
        let ty = etl_type_of(&items[1]).ok()??;
        return Some(vec![ColumnTypeSpec {
            column: text_of(&items[0])?,
            ty,
        }]);
    }
    items
        .iter()
        .map(|p| {
            let pair = p.unparen().as_list()?;
            if pair.len() != 2 {
                return None;
            }
            Some(ColumnTypeSpec {
                column: text_of(&pair[0])?,
                ty: etl_type_of(&pair[1]).ok()??,
            })
        })
        .collect()
}

fn culture_arg(e: Option<&Expr>) -> Option<Option<String>> {
    match e {
        None => Some(None),
        Some(x) if x.is_null() => Some(None),
        Some(x) => match &x.unparen().kind {
            ExprKind::Text(t) => Some(Some(t.clone())),
            // [Culture="pt-BR"] options record
            ExprKind::Record(fields) => {
                let mut culture = None;
                for (k, v) in fields {
                    if k == "Culture" {
                        culture = Some(text_of(v)?);
                    } else {
                        return None;
                    }
                }
                Some(culture)
            }
            _ => None,
        },
    }
}

impl<'a> Converter<'a> {
    fn is_binding(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Identifier naming another query (not a binding of this let).
    fn query_ref(&self, e: &Expr) -> Option<String> {
        let n = e.unparen().as_ident()?;
        if self.is_binding(n) || n.starts_with('#') || n == "_" {
            return None;
        }
        Some(n.to_string())
    }

    // -------------------------------------------------------------- sources

    fn recognize_source(&self, b: &[Binding], warnings: &mut Vec<String>) -> SourceMatch {
        let b0 = &b[0];
        let one = |source: EtlSource| SourceMatch {
            source,
            consumed: 1,
            final_name: b0.name.clone(),
        };

        // Excel.Workbook(File.Contents(p), null, true) [+ X{[Item=..,Kind="Sheet"]}[Data]]
        if let Some(path) = self.excel_workbook_path(&b0.expr) {
            if let Some(b1) = b.get(1) {
                if let Some((target, sheet)) = Self::sheet_navigation(&b1.expr) {
                    if target.unparen().as_ident() == Some(b0.name.as_str()) {
                        return SourceMatch {
                            source: EtlSource::excel(path, sheet),
                            consumed: 2,
                            final_name: b1.name.clone(),
                        };
                    }
                }
            }
        }
        if let Some((target, sheet)) = Self::sheet_navigation(&b0.expr) {
            if let Some(path) = self.excel_workbook_path(target) {
                return one(EtlSource::excel(path, sheet));
            }
        }

        if let Some((f, args)) = b0.expr.unparen().as_call() {
            match f {
                "Csv.Document" if !args.is_empty() && args.len() <= 2 => {
                    if let Some(p) = file_contents_path(&args[0]) {
                        return one(EtlSource::csv(p));
                    }
                }
                "Parquet.Document" if args.len() == 1 => {
                    if let Some(p) = file_contents_path(&args[0]) {
                        let mut s = EtlSource::empty(SourceKind::Parquet);
                        s.path = Some(p);
                        return one(s);
                    }
                }
                "Table.FromRecords" if args.len() == 1 => {
                    if let Some(("Json.Document", [fc])) = args[0].unparen().as_call() {
                        if let Some(p) = file_contents_path(fc) {
                            let mut s = EtlSource::empty(SourceKind::Json);
                            s.path = Some(p);
                            return one(s);
                        }
                    }
                }
                "Json.Document" if args.len() == 1 => {
                    if let Some(p) = file_contents_path(&args[0]) {
                        let mut s = EtlSource::empty(SourceKind::Json);
                        s.path = Some(p);
                        // optional Table.FromRecords(Source)
                        if let Some(b1) = b.get(1) {
                            if let Some(("Table.FromRecords", [x])) = b1.expr.unparen().as_call() {
                                if x.unparen().as_ident() == Some(b0.name.as_str()) {
                                    return SourceMatch {
                                        source: s,
                                        consumed: 2,
                                        final_name: b1.name.clone(),
                                    };
                                }
                            }
                        }
                        return one(s);
                    }
                }
                "PrismBI.DbTable" if args.len() == 1 => {
                    if let Some(t) = text_of(&args[0]) {
                        let mut s = EtlSource::empty(SourceKind::Db);
                        s.table = Some(t);
                        return one(s);
                    }
                }
                _ => {}
            }
            if let Some(m) = self.list_dates_source(b, warnings) {
                return m;
            }
        }

        if let Some(t) = self.query_ref(&b0.expr) {
            return one(EtlSource::table_ref(t));
        }

        if let Some(m) = self.inline_source(b) {
            return m;
        }

        warnings.push(format!(
            "Fonte '{}' ({}) não reconhecida; mantida como M opaco (não executável).",
            b0.name,
            describe_head(&b0.expr)
        ));
        one(EtlSource::opaque(b0.expr.text(self.src)))
    }

    fn excel_workbook_path(&self, e: &Expr) -> Option<String> {
        let (f, args) = e.unparen().as_call()?;
        if f != "Excel.Workbook" || args.is_empty() || args.len() > 3 {
            return None;
        }
        // useHeaders = true would promote headers at the source: not modelled.
        if let Some(h) = args.get(1) {
            if !(h.is_null() || matches!(h.unparen().kind, ExprKind::Bool(false))) {
                return None;
            }
        }
        file_contents_path(&args[0])
    }

    /// `X{[Item="S",Kind="Sheet"]}[Data]` → (X, "S")
    fn sheet_navigation(e: &Expr) -> Option<(&Expr, String)> {
        let ExprKind::Field {
            target: Some(t),
            name,
            ..
        } = &e.unparen().kind
        else {
            return None;
        };
        if name != "Data" {
            return None;
        }
        let ExprKind::Item { target, key, .. } = &t.kind else {
            return None;
        };
        let ExprKind::Record(fields) = &key.unparen().kind else {
            return None;
        };
        let mut item = None;
        for (k, v) in fields {
            match k.as_str() {
                "Item" | "Name" => item = Some(text_of(v)?),
                "Kind" => {
                    if text_of(v)? != "Sheet" {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some((target, item?))
    }

    fn list_dates_source(&self, b: &[Binding], warnings: &mut Vec<String>) -> Option<SourceMatch> {
        let b0 = &b[0];
        let b1 = b.get(1)?;
        let (callee, args) = b0.expr.unparen().as_call()?;
        if args.len() != 3 || (callee != "List.Dates" && (self.is_binding(callee) || callee.starts_with('#'))) {
            return None;
        }
        let (dfn, dargs) = args[0].unparen().as_call()?;
        if dfn != "#date" || dargs.len() != 3 {
            return None;
        }
        let y = i64_of(&dargs[0])?;
        let mo = i64_of(&dargs[1])?;
        let d = i64_of(&dargs[2])?;
        let count = u64_of(&args[1])?;
        let (durfn, durargs) = args[2].unparen().as_call()?;
        if durfn != "#duration" || durargs.len() != 4 {
            return None;
        }
        let step = i64_of(&durargs[0])?;
        if durargs[1..].iter().any(|a| a.as_number() != Some(0.0)) {
            return None;
        }
        // Table.FromList(Source, Splitter.SplitByNothing(), null, null, ExtraValues.Error)
        let (tf, targs) = b1.expr.unparen().as_call()?;
        if tf != "Table.FromList" || targs.len() < 2 || targs[0].unparen().as_ident() != Some(b0.name.as_str()) {
            return None;
        }
        match targs[1].unparen().as_call() {
            Some(("Splitter.SplitByNothing", [])) => {}
            _ => return None,
        }
        if targs.len() > 2 && !targs[2].is_null() {
            return None;
        }
        if targs.len() > 3 && !targs[3].is_null() {
            return None;
        }
        if targs.len() > 4 && targs[4].unparen().as_ident() != Some("ExtraValues.Error") {
            return None;
        }
        if targs.len() > 5 {
            return None;
        }
        let mut s = EtlSource::empty(SourceKind::ListDates);
        s.start = Some(format!("{:04}-{:02}-{:02}", y, mo, d));
        s.count = Some(count);
        s.step_days = Some(step);
        if callee != "List.Dates" {
            s.function = Some(callee.to_string());
            warnings.push(format!(
                "Fonte '{}': a função '{}' foi tratada como List.Dates (mesma assinatura: data, quantidade, #duration).",
                b0.name, callee
            ));
        }
        Some(SourceMatch {
            source: s,
            consumed: 2,
            final_name: b1.name.clone(),
        })
    }

    /// Literal bindings followed by `#table(cols, rows)` whose cells fold to constants.
    fn inline_source(&self, b: &[Binding]) -> Option<SourceMatch> {
        let mut env: HashMap<String, Scalar> = HashMap::new();
        for (k, bind) in b.iter().enumerate().take(8) {
            if let Some(v) = const_eval(&bind.expr, &env) {
                env.insert(bind.name.clone(), v);
                continue;
            }
            let (f, args) = bind.expr.unparen().as_call()?;
            if f != "#table" || args.len() != 2 {
                return None;
            }
            let rows: Vec<Vec<Scalar>> = args[1]
                .unparen()
                .as_list()?
                .iter()
                .map(|r| {
                    r.unparen()
                        .as_list()?
                        .iter()
                        .map(|c| const_eval(c, &env))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<_>>()?;
            let columns: Vec<String> = match &args[0].unparen().kind {
                ExprKind::List(_) => text_list(&args[0])?,
                _ => {
                    let n = u64_of(&args[0])?;
                    (1..=n).map(|i| format!("Column{}", i)).collect()
                }
            };
            if rows.iter().any(|r| r.len() != columns.len()) {
                return None;
            }
            return Some(SourceMatch {
                source: EtlSource::inline(columns, rows),
                consumed: k + 1,
                final_name: bind.name.clone(),
            });
        }
        None
    }

    // ---------------------------------------------------------------- steps

    /// Table argument of a step: `Ok(vec![])` if it is `prev`, or the extra
    /// steps produced by a nested table call.
    fn input_steps(&self, outer_name: &str, arg: &Expr, prev: &str) -> Option<Vec<(String, StepKind)>> {
        if arg.unparen().as_ident() == Some(prev) {
            return Some(vec![]);
        }
        if arg.unparen().as_call().is_some() {
            let kind_hint = self.convert_step("", arg.unparen(), prev)?;
            let last = kind_hint.last()?.1.default_step_name();
            let inner_name = self.fresh_name(&format!("{} - {}", outer_name, last));
            return self.convert_step(&inner_name, arg.unparen(), prev);
        }
        None
    }

    fn fresh_name(&self, base: &str) -> String {
        if !self.is_binding(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| format!("{} ({})", base, i))
            .find(|n| !self.is_binding(n))
            .unwrap()
    }

    fn merge_two_bindings(&self, join: &Expr, join_name: &str, expand: &Expr, prev: &str) -> Option<StepKind> {
        let (f, args) = expand.unparen().as_call()?;
        if f != "Table.ExpandTableColumn" || args[0].unparen().as_ident() != Some(join_name) {
            return None;
        }
        let (rq, lk, rk, jk, jcol) = self.nested_join(join, prev)?;
        let expand = self.expand_args(&args[1..], &jcol)?;
        Some(StepKind::MergeQueries {
            right_query: rq,
            left_keys: lk,
            right_keys: rk,
            join_kind: jk,
            expand,
        })
    }

    #[allow(clippy::type_complexity)]
    fn nested_join(&self, e: &Expr, prev: &str) -> Option<(String, Vec<String>, Vec<String>, JoinKind, String)> {
        let (f, args) = e.unparen().as_call()?;
        if f != "Table.NestedJoin" || !(5..=6).contains(&args.len()) {
            return None;
        }
        if args[0].unparen().as_ident() != Some(prev) {
            return None;
        }
        let lk = text_list(&args[1])?;
        let rq = self.query_ref(&args[2])?;
        let rk = text_list(&args[3])?;
        let jcol = text_of(&args[4])?;
        let jk = match args.get(5) {
            Some(j) => join_kind_of(j)?,
            None => JoinKind::LeftOuter,
        };
        if lk.len() != rk.len() || lk.is_empty() {
            return None;
        }
        Some((rq, lk, rk, jk, jcol))
    }

    /// ExpandTableColumn args after the table: (joinCol, cols, names?)
    fn expand_args(&self, args: &[Expr], jcol: &str) -> Option<Vec<ExpandColumn>> {
        if args.len() < 2 || args.len() > 3 || text_of(&args[0])? != jcol {
            return None;
        }
        let cols = text_list(&args[1])?;
        let names = match args.get(2) {
            Some(n) => text_list(n)?,
            None => cols.clone(),
        };
        if names.len() != cols.len() {
            return None;
        }
        Some(
            cols.into_iter()
                .zip(names)
                .map(|(column, as_name)| ExpandColumn { column, as_name })
                .collect(),
        )
    }

    /// One binding → one or more steps (nested calls are unrolled).
    fn convert_step(&self, name: &str, expr: &Expr, prev: &str) -> Option<Vec<(String, StepKind)>> {
        let (f, args) = expr.unparen().as_call()?;
        if args.is_empty() {
            return None;
        }
        let name = name.to_string();

        // Steps whose input is not args[0] or that need special input handling.
        match f {
            "Table.Combine" => {
                if args.len() != 1 {
                    return None;
                }
                let items = args[0].unparen().as_list()?;
                if items.len() < 2 || items[0].unparen().as_ident() != Some(prev) {
                    return None;
                }
                let queries = items[1..]
                    .iter()
                    .map(|q| self.query_ref(q))
                    .collect::<Option<Vec<_>>>()?;
                return Some(vec![(name, StepKind::AppendQueries { queries })]);
            }
            "Table.ExpandTableColumn" => {
                let (rq, lk, rk, jk, jcol) = self.nested_join(&args[0], prev)?;
                let expand = self.expand_args(&args[1..], &jcol)?;
                return Some(vec![(
                    name,
                    StepKind::MergeQueries {
                        right_query: rq,
                        left_keys: lk,
                        right_keys: rk,
                        join_kind: jk,
                        expand,
                    },
                )]);
            }
            "Table.CombineColumns" => {
                if args.len() != 4 {
                    return None;
                }
                let columns = text_list(&args[1])?;
                let (cf, cargs) = args[2].unparen().as_call()?;
                if cf != "Combiner.CombineTextByDelimiter" || cargs.is_empty() || cargs.len() > 2 {
                    return None;
                }
                if !quote_style_ok(cargs.get(1)) {
                    return None;
                }
                let separator = text_of(&cargs[0])?;
                let new_column = text_of(&args[3])?;
                // The UI wraps the input in TransformColumnTypes(.., {{c, type text}}) for
                // non-text columns; without a culture that is exactly what mergeColumns
                // already does, so it is absorbed. With a culture it is kept as a step.
                let mut pre = Vec::new();
                let absorbed = match args[0].unparen().as_call() {
                    Some(("Table.TransformColumnTypes", targs))
                        if targs.len() == 2 && targs[0].unparen().as_ident() == Some(prev) =>
                    {
                        match change_type_specs(&targs[1]) {
                            Some(specs) => specs
                                .iter()
                                .all(|s| s.ty == EtlType::Text && columns.contains(&s.column)),
                            None => false,
                        }
                    }
                    _ => false,
                };
                if !absorbed {
                    pre = self.input_steps(&name, &args[0], prev)?;
                }
                pre.push((
                    name,
                    StepKind::MergeColumns {
                        columns,
                        separator,
                        new_column,
                    },
                ));
                return Some(pre);
            }
            _ => {}
        }

        let mut steps = self.input_steps(&name, &args[0], prev)?;
        let a = &args[1..];
        let kind = match (f, a) {
            ("Table.PromoteHeaders", []) => StepKind::PromoteHeaders,
            ("Table.PromoteHeaders", [opts]) => match &opts.unparen().kind {
                ExprKind::Record(fields)
                    if fields.iter().all(|(k, v)| {
                        k == "PromoteAllScalars" && matches!(v.unparen().kind, ExprKind::Bool(_))
                            || k == "Culture" && text_of(v).is_some()
                    }) =>
                {
                    StepKind::PromoteHeaders
                }
                _ if opts.is_null() => StepKind::PromoteHeaders,
                _ => return None,
            },
            ("Table.TransformColumnTypes", [specs]) | ("Table.TransformColumnTypes", [specs, _]) => {
                let culture = culture_arg(a.get(1))?;
                StepKind::ChangeType {
                    columns: change_type_specs(specs)?,
                    culture,
                }
            }
            ("Table.RenameColumns", [r]) | ("Table.RenameColumns", [r, _]) => {
                if let Some(extra) = a.get(1) {
                    if extra.unparen().as_ident() != Some("MissingField.Error") {
                        return None;
                    }
                }
                let items = r.unparen().as_list()?;
                let renames = if items.len() == 2 && items[0].unparen().as_list().is_none() {
                    vec![(text_of(&items[0])?, text_of(&items[1])?)]
                } else {
                    items
                        .iter()
                        .map(|p| {
                            let pair = p.unparen().as_list()?;
                            if pair.len() != 2 {
                                return None;
                            }
                            Some((text_of(&pair[0])?, text_of(&pair[1])?))
                        })
                        .collect::<Option<Vec<_>>>()?
                };
                StepKind::RenameColumns { renames }
            }
            ("Table.RemoveColumns", [c]) => StepKind::RemoveColumns { columns: text_list(c)? },
            ("Table.SelectColumns", [c]) => StepKind::SelectColumns { columns: text_list(c)? },
            ("Table.DuplicateColumn", [c, n]) | ("Table.DuplicateColumn", [c, n, _]) => {
                if let Some(t) = a.get(2) {
                    etl_type_of(t).ok()?;
                }
                StepKind::DuplicateColumn {
                    column: text_of(c)?,
                    new_column: text_of(n)?,
                }
            }
            ("Table.SelectRows", [pred]) => {
                let ExprKind::Each(body) = &pred.unparen().kind else {
                    return None;
                };
                if is_remove_blank_rows(body) {
                    StepKind::RemoveBlankRows
                } else {
                    StepKind::FilterRows {
                        condition: cond_of(body)?,
                    }
                }
            }
            ("Table.RemoveRowsWithErrors", []) => StepKind::RemoveRowsWithErrors { columns: None },
            ("Table.RemoveRowsWithErrors", [c]) => StepKind::RemoveRowsWithErrors {
                columns: Some(text_list(c)?),
            },
            ("Table.FirstN", [n]) => StepKind::KeepTopRows { count: u64_of(n)? },
            ("Table.Skip", [n]) | ("Table.RemoveFirstN", [n]) => StepKind::RemoveTopRows { count: u64_of(n)? },
            ("Table.LastN", [n]) => StepKind::KeepBottomRows { count: u64_of(n)? },
            ("Table.RemoveLastN", [n]) => StepKind::RemoveBottomRows { count: u64_of(n)? },
            ("Table.Distinct", []) => StepKind::RemoveDuplicates { columns: None },
            ("Table.Distinct", [c]) => StepKind::RemoveDuplicates {
                columns: Some(text_list(c)?),
            },
            ("Table.Sort", [keys]) => {
                let items = keys.unparen().as_list()?;
                let mut by = Vec::new();
                for k in items {
                    match &k.unparen().kind {
                        ExprKind::Text(c) => by.push(SortKey {
                            column: c.clone(),
                            descending: false,
                        }),
                        ExprKind::List(pair) if pair.len() == 2 => {
                            let descending = match pair[1].unparen().as_ident() {
                                Some("Order.Ascending") => false,
                                Some("Order.Descending") => true,
                                _ => match pair[1].as_number() {
                                    Some(0.0) => false,
                                    Some(1.0) => true,
                                    _ => return None,
                                },
                            };
                            by.push(SortKey {
                                column: text_of(&pair[0])?,
                                descending,
                            });
                        }
                        _ => return None,
                    }
                }
                StepKind::Sort { by }
            }
            ("Table.ReplaceValue", [find, replace, replacer, cols]) => {
                let match_entire_cell = match replacer.unparen().as_ident()? {
                    "Replacer.ReplaceText" => false,
                    "Replacer.ReplaceValue" => true,
                    _ => return None,
                };
                StepKind::ReplaceValues {
                    columns: text_list(cols)?,
                    find: scalar_of(find)?,
                    replace: scalar_of(replace)?,
                    match_entire_cell,
                }
            }
            ("Table.FillDown", [c]) => StepKind::FillDown { columns: text_list(c)? },
            ("Table.FillUp", [c]) => StepKind::FillUp { columns: text_list(c)? },
            ("Table.SplitColumn", [col, splitter, into]) => {
                let column = text_of(col)?;
                let (mode, delimiter, positions, length) = splitter_of(splitter)?;
                let into = match text_list(into) {
                    Some(v) => v,
                    None => {
                        let n = u64_of(into)?;
                        (1..=n).map(|i| format!("{}.{}", column, i)).collect()
                    }
                };
                StepKind::SplitColumn {
                    column,
                    mode,
                    delimiter,
                    positions,
                    length,
                    into,
                }
            }
            ("Table.TransformColumns", [specs]) => {
                let items = specs.unparen().as_list()?;
                let pairs: Vec<&[Expr]> = if items.len() >= 2 && text_of(&items[0]).is_some() {
                    vec![items]
                } else {
                    items.iter().map(|p| p.unparen().as_list()).collect::<Option<_>>()?
                };
                let mut func = None;
                let mut columns = Vec::new();
                for p in pairs {
                    if p.len() < 2 || p.len() > 3 {
                        return None;
                    }
                    let f = text_fn_of(&p[1])?;
                    if let Some(t) = p.get(2) {
                        if etl_type_of(t).ok()? != Some(EtlType::Text) {
                            return None;
                        }
                    }
                    if func.is_some_and(|x| x != f) {
                        return None;
                    }
                    func = Some(f);
                    columns.push(text_of(&p[0])?);
                }
                StepKind::TextTransform { columns, func: func? }
            }
            ("Table.UnpivotOtherColumns", [keep, attr, val]) => StepKind::UnpivotOtherColumns {
                keep: text_list(keep)?,
                attribute_column: text_of(attr)?,
                value_column: text_of(val)?,
            },
            ("Table.Unpivot", [cols, attr, val]) => StepKind::UnpivotColumns {
                columns: text_list(cols)?,
                attribute_column: text_of(attr)?,
                value_column: text_of(val)?,
            },
            ("Table.Pivot", [values, col, vcol, aggf]) => {
                let column = text_of(col)?;
                // values must be List.Distinct(prev[column])
                let (lf, largs) = values.unparen().as_call()?;
                if lf != "List.Distinct" || largs.len() != 1 {
                    return None;
                }
                match &largs[0].unparen().kind {
                    ExprKind::Field {
                        target: Some(t),
                        name: c,
                        ..
                    } if t.unparen().as_ident() == Some(prev) && *c == column => {}
                    _ => return None,
                }
                StepKind::Pivot {
                    column,
                    values_column: text_of(vcol)?,
                    aggregation: agg_list_fn(aggf)?,
                }
            }
            ("Table.Group", [keys, aggs]) => StepKind::GroupBy {
                keys: text_list(keys)?,
                aggregations: {
                    let items = aggs.unparen().as_list()?;
                    // single aggregation written as {"n", each .., type}
                    if items.len() >= 2 && text_of(&items[0]).is_some() {
                        vec![group_agg(aggs)?]
                    } else {
                        items.iter().map(group_agg).collect::<Option<Vec<_>>>()?
                    }
                },
            },
            ("Table.AddColumn", [col, func]) | ("Table.AddColumn", [col, func, _]) => {
                let new_column = text_of(col)?;
                let ty = match a.get(2) {
                    Some(t) => etl_type_of(t).ok()?,
                    None => None,
                };
                let ExprKind::Each(body) = &func.unparen().kind else {
                    return None;
                };
                match conditional_of(body) {
                    Some((rules, otherwise)) => StepKind::AddConditionalColumn {
                        new_column,
                        rules,
                        otherwise,
                        ty,
                    },
                    None => StepKind::AddCustomColumn {
                        new_column,
                        expression: body.text(self.src).to_string(),
                        ty,
                    },
                }
            }
            ("Table.AddIndexColumn", [col]) => StepKind::AddIndexColumn {
                new_column: text_of(col)?,
                start: 0,
                step: 1,
            },
            ("Table.AddIndexColumn", [col, start, step]) | ("Table.AddIndexColumn", [col, start, step, _]) => {
                if let Some(t) = a.get(3) {
                    etl_type_of(t).ok()?;
                }
                StepKind::AddIndexColumn {
                    new_column: text_of(col)?,
                    start: i64_of(start)?,
                    step: i64_of(step)?,
                }
            }
            _ => return None,
        };
        steps.push((name, kind));
        Some(steps)
    }
}

/// Constant folding for literal bindings (`Source = "Medidas="`).
fn const_eval(e: &Expr, env: &HashMap<String, Scalar>) -> Option<Scalar> {
    if let Some(s) = scalar_of(e) {
        return Some(s);
    }
    match &e.unparen().kind {
        ExprKind::Ident(n) => env.get(n).cloned(),
        _ => None,
    }
}
