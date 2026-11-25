//! `EtlQuery` → idiomatic Power Query M (`let … in …`).

use super::m_parse::{format_m_number, quote_m_ident, quote_m_string};
use super::model::*;
use std::collections::HashSet;

pub fn m_type(t: EtlType) -> &'static str {
    match t {
        EtlType::Text => "type text",
        EtlType::Integer => "Int64.Type",
        EtlType::Decimal => "type number",
        EtlType::Currency => "Currency.Type",
        EtlType::Date => "type date",
        EtlType::Datetime => "type datetime",
        EtlType::Boolean => "type logical",
    }
}

pub fn m_scalar(s: &Scalar) -> String {
    match s {
        Scalar::Null => "null".into(),
        Scalar::Bool(b) => if *b { "true".into() } else { "false".into() },
        Scalar::Number(n) => format_m_number(*n),
        Scalar::Text(t) => quote_m_string(t),
    }
}

fn is_generalized_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    if !(first.is_alphabetic() || first == '_') {
        return false;
    }
    if name.ends_with(' ') || name.ends_with('.') || name.contains("  ") || name.contains("..") {
        return false;
    }
    name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ' ' || c == '.')
}

/// `[Col]` field access (generalized identifier or `#"..."`).
pub fn m_field(name: &str) -> String {
    if is_generalized_ident(name) {
        format!("[{}]", name)
    } else {
        format!("[{}]", quote_m_ident_forced(name))
    }
}

fn quote_m_ident_forced(name: &str) -> String {
    format!("#{}", quote_m_string(name))
}

fn m_text_list(items: &[String]) -> String {
    format!(
        "{{{}}}",
        items.iter().map(|s| quote_m_string(s)).collect::<Vec<_>>().join(", ")
    )
}

fn m_scalar_list(items: &[Scalar]) -> String {
    format!("{{{}}}", items.iter().map(m_scalar).collect::<Vec<_>>().join(", "))
}

fn cond_is_compound(c: &Cond) -> bool {
    matches!(
        c,
        Cond::And { .. }
            | Cond::Or { .. }
            | Cond::Compare(CompareCond {
                op: CondOp::IsBlank | CondOp::IsNotBlank,
                ..
            })
    )
}

fn m_cond_child(c: &Cond) -> Result<String, String> {
    let s = m_cond(c)?;
    Ok(if cond_is_compound(c) { format!("({})", s) } else { s })
}

/// Row predicate as the body of an `each` (`[Col] > 5 and …`).
pub fn m_cond(c: &Cond) -> Result<String, String> {
    Ok(match c {
        Cond::And { and } | Cond::Or { or: and } => {
            if and.is_empty() {
                return Err("condição 'and'/'or' vazia".into());
            }
            let sep = if matches!(c, Cond::And { .. }) { " and " } else { " or " };
            and.iter()
                .map(m_cond_child)
                .collect::<Result<Vec<_>, _>>()?
                .join(sep)
        }
        Cond::Not { not } => format!("not ({})", m_cond(not)?),
        Cond::Compare(CompareCond { column, op }) => {
            let f = m_field(column);
            match op {
                CondOp::Eq { value } => format!("{} = {}", f, m_scalar(value)),
                CondOp::Neq { value } => format!("{} <> {}", f, m_scalar(value)),
                CondOp::Gt { value } => format!("{} > {}", f, m_scalar(value)),
                CondOp::Gte { value } => format!("{} >= {}", f, m_scalar(value)),
                CondOp::Lt { value } => format!("{} < {}", f, m_scalar(value)),
                CondOp::Lte { value } => format!("{} <= {}", f, m_scalar(value)),
                CondOp::Contains { value } => format!("Text.Contains({}, {})", f, m_scalar(value)),
                CondOp::NotContains { value } => {
                    format!("not Text.Contains({}, {})", f, m_scalar(value))
                }
                CondOp::StartsWith { value } => format!("Text.StartsWith({}, {})", f, m_scalar(value)),
                CondOp::EndsWith { value } => format!("Text.EndsWith({}, {})", f, m_scalar(value)),
                CondOp::IsNull => format!("{} = null", f),
                CondOp::IsNotNull => format!("{} <> null", f),
                CondOp::IsBlank => format!("{} = null or {} = \"\"", f, f),
                CondOp::IsNotBlank => format!("{} <> null and {} <> \"\"", f, f),
                CondOp::In { values } => format!("List.Contains({}, {})", m_scalar_list(values), f),
                CondOp::NotIn { values } => {
                    format!("not List.Contains({}, {})", m_scalar_list(values), f)
                }
            }
        }
    })
}

fn m_value_or_column(v: &ValueOrColumn) -> String {
    match v {
        ValueOrColumn::Column { column } => m_field(column),
        ValueOrColumn::Value(s) => m_scalar(s),
    }
}

fn m_agg_list_fn(op: AggOp) -> &'static str {
    match op {
        AggOp::Sum => "List.Sum",
        AggOp::Avg => "List.Average",
        AggOp::Min => "List.Min",
        AggOp::Max => "List.Max",
        AggOp::Count => "List.NonNullCount",
        AggOp::CountRows => "List.Count",
        AggOp::Median => "List.Median",
        AggOp::CountDistinct => "each List.Count(List.Distinct(_))",
    }
}

fn m_group_agg(a: &Agg) -> Result<String, String> {
    let col = || {
        a.column
            .as_deref()
            .map(m_field)
            .ok_or_else(|| format!("agregação '{}' precisa de uma coluna", a.name))
    };
    let (body, ty) = match a.op {
        AggOp::Sum => (format!("List.Sum({})", col()?), "type number"),
        AggOp::Avg => (format!("List.Average({})", col()?), "type number"),
        AggOp::Median => (format!("List.Median({})", col()?), "type number"),
        AggOp::Min => (format!("List.Min({})", col()?), "type any"),
        AggOp::Max => (format!("List.Max({})", col()?), "type any"),
        AggOp::Count => match &a.column {
            Some(c) => (format!("List.NonNullCount({})", m_field(c)), "Int64.Type"),
            None => ("Table.RowCount(_)".to_string(), "Int64.Type"),
        },
        AggOp::CountRows => ("Table.RowCount(_)".to_string(), "Int64.Type"),
        AggOp::CountDistinct => match &a.column {
            Some(c) => (format!("List.Count(List.Distinct({}))", m_field(c)), "Int64.Type"),
            None => ("Table.RowCount(Table.Distinct(_))".to_string(), "Int64.Type"),
        },
    };
    Ok(format!("{{{}, each {}, {}}}", quote_m_string(&a.name), body, ty))
}

fn m_splitter(
    mode: SplitMode,
    delimiter: &Option<String>,
    positions: &Option<Vec<u64>>,
    length: &Option<u64>,
) -> String {
    let d = quote_m_string(delimiter.as_deref().unwrap_or(""));
    match mode {
        SplitMode::DelimiterEach => format!("Splitter.SplitTextByDelimiter({}, QuoteStyle.Csv)", d),
        SplitMode::DelimiterLeftmost => {
            format!("Splitter.SplitTextByEachDelimiter({{{}}}, QuoteStyle.Csv, false)", d)
        }
        SplitMode::DelimiterRightmost => {
            format!("Splitter.SplitTextByEachDelimiter({{{}}}, QuoteStyle.Csv, true)", d)
        }
        SplitMode::Positions => format!(
            "Splitter.SplitTextByPositions({{{}}})",
            positions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SplitMode::FixedLength => {
            format!("Splitter.SplitTextByRepeatedLengths({})", length.unwrap_or(1))
        }
    }
}

fn text_fn_name(f: TextFn) -> &'static str {
    match f {
        TextFn::Upper => "Text.Upper",
        TextFn::Lower => "Text.Lower",
        TextFn::Proper => "Text.Proper",
        TextFn::Trim => "Text.Trim",
        TextFn::Clean => "Text.Clean",
    }
}

/// Custom-column formula as written by the user, with or without `each`.
fn each_body(expression: &str) -> String {
    let t = expression.trim();
    let has_each = t.strip_prefix("each").is_some_and(|rest| {
        rest.chars().next().is_some_and(|c| c.is_whitespace() || c == '(' || c == '[')
    });
    if has_each {
        t.to_string()
    } else {
        format!("each {}", t)
    }
}

/// M expression of one step, reading from `prev` (already-quoted identifier).
pub fn m_step_expr(kind: &StepKind, prev: &str) -> Result<String, String> {
    Ok(match kind {
        StepKind::PromoteHeaders => format!("Table.PromoteHeaders({}, [PromoteAllScalars=true])", prev),
        StepKind::ChangeType { columns, culture } => {
            let specs = columns
                .iter()
                .map(|c| format!("{{{}, {}}}", quote_m_string(&c.column), m_type(c.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            match culture {
                Some(cu) => format!("Table.TransformColumnTypes({},{{{}}}, {})", prev, specs, quote_m_string(cu)),
                None => format!("Table.TransformColumnTypes({},{{{}}})", prev, specs),
            }
        }
        StepKind::RenameColumns { renames } => format!(
            "Table.RenameColumns({},{{{}}})",
            prev,
            renames
                .iter()
                .map(|(a, b)| format!("{{{}, {}}}", quote_m_string(a), quote_m_string(b)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        StepKind::RemoveColumns { columns } => format!("Table.RemoveColumns({},{})", prev, m_text_list(columns)),
        StepKind::SelectColumns { columns } => format!("Table.SelectColumns({},{})", prev, m_text_list(columns)),
        StepKind::DuplicateColumn { column, new_column } => format!(
            "Table.DuplicateColumn({}, {}, {})",
            prev,
            quote_m_string(column),
            quote_m_string(new_column)
        ),
        StepKind::FilterRows { condition } => format!("Table.SelectRows({}, each {})", prev, m_cond(condition)?),
        StepKind::RemoveBlankRows => format!(
            "Table.SelectRows({}, each not List.IsEmpty(List.RemoveMatchingItems(Record.FieldValues(_), {{\"\", null}})))",
            prev
        ),
        StepKind::RemoveRowsWithErrors { columns } => match columns {
            Some(cols) => format!("Table.RemoveRowsWithErrors({}, {})", prev, m_text_list(cols)),
            None => format!("Table.RemoveRowsWithErrors({})", prev),
        },
        StepKind::KeepTopRows { count } => format!("Table.FirstN({},{})", prev, count),
        StepKind::RemoveTopRows { count } => format!("Table.Skip({},{})", prev, count),
        StepKind::KeepBottomRows { count } => format!("Table.LastN({},{})", prev, count),
        StepKind::RemoveBottomRows { count } => format!("Table.RemoveLastN({},{})", prev, count),
        StepKind::RemoveDuplicates { columns } => match columns {
            Some(cols) => format!("Table.Distinct({}, {})", prev, m_text_list(cols)),
            None => format!("Table.Distinct({})", prev),
        },
        StepKind::Sort { by } => format!(
            "Table.Sort({},{{{}}})",
            prev,
            by.iter()
                .map(|k| format!(
                    "{{{}, {}}}",
                    quote_m_string(&k.column),
                    if k.descending { "Order.Descending" } else { "Order.Ascending" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        StepKind::ReplaceValues {
            columns,
            find,
            replace,
            match_entire_cell,
        } => format!(
            "Table.ReplaceValue({},{},{},{},{})",
            prev,
            m_scalar(find),
            m_scalar(replace),
            if *match_entire_cell { "Replacer.ReplaceValue" } else { "Replacer.ReplaceText" },
            m_text_list(columns)
        ),
        StepKind::FillDown { columns } => format!("Table.FillDown({},{})", prev, m_text_list(columns)),
        StepKind::FillUp { columns } => format!("Table.FillUp({},{})", prev, m_text_list(columns)),
        StepKind::SplitColumn {
            column,
            mode,
            delimiter,
            positions,
            length,
            into,
        } => format!(
            "Table.SplitColumn({}, {}, {}, {})",
            prev,
            quote_m_string(column),
            m_splitter(*mode, delimiter, positions, length),
            m_text_list(into)
        ),
        StepKind::MergeColumns {
            columns,
            separator,
            new_column,
        } => {
            let to_text = columns
                .iter()
                .map(|c| format!("{{{}, type text}}", quote_m_string(c)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Table.CombineColumns(Table.TransformColumnTypes({}, {{{}}}),{},Combiner.CombineTextByDelimiter({}, QuoteStyle.None),{})",
                prev,
                to_text,
                m_text_list(columns),
                quote_m_string(separator),
                quote_m_string(new_column)
            )
        }
        StepKind::TextTransform { columns, func } => format!(
            "Table.TransformColumns({},{{{}}})",
            prev,
            columns
                .iter()
                .map(|c| format!("{{{}, {}, type text}}", quote_m_string(c), text_fn_name(*func)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        StepKind::UnpivotOtherColumns {
            keep,
            attribute_column,
            value_column,
        } => format!(
            "Table.UnpivotOtherColumns({}, {}, {}, {})",
            prev,
            m_text_list(keep),
            quote_m_string(attribute_column),
            quote_m_string(value_column)
        ),
        StepKind::UnpivotColumns {
            columns,
            attribute_column,
            value_column,
        } => format!(
            "Table.Unpivot({}, {}, {}, {})",
            prev,
            m_text_list(columns),
            quote_m_string(attribute_column),
            quote_m_string(value_column)
        ),
        StepKind::Pivot {
            column,
            values_column,
            aggregation,
        } => format!(
            "Table.Pivot({}, List.Distinct({}{}), {}, {}, {})",
            prev,
            prev,
            m_field(column),
            quote_m_string(column),
            quote_m_string(values_column),
            m_agg_list_fn(*aggregation)
        ),
        StepKind::GroupBy { keys, aggregations } => format!(
            "Table.Group({}, {}, {{{}}})",
            prev,
            m_text_list(keys),
            aggregations
                .iter()
                .map(m_group_agg)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        ),
        StepKind::AddConditionalColumn {
            new_column,
            rules,
            otherwise,
            ty,
        } => {
            if rules.is_empty() {
                return Err(format!("coluna condicional '{}' sem regras", new_column));
            }
            let mut body = String::new();
            for (i, r) in rules.iter().enumerate() {
                if i > 0 {
                    body.push_str(" else ");
                }
                body.push_str(&format!("if {} then {}", m_cond(&r.when)?, m_value_or_column(&r.then)));
            }
            body.push_str(&format!(" else {}", m_value_or_column(otherwise)));
            match ty {
                Some(t) => format!("Table.AddColumn({}, {}, each {}, {})", prev, quote_m_string(new_column), body, m_type(*t)),
                None => format!("Table.AddColumn({}, {}, each {})", prev, quote_m_string(new_column), body),
            }
        }
        StepKind::AddCustomColumn {
            new_column,
            expression,
            ty,
        } => {
            if expression.trim().is_empty() {
                return Err(format!("coluna personalizada '{}' sem fórmula", new_column));
            }
            match ty {
                Some(t) => format!(
                    "Table.AddColumn({}, {}, {}, {})",
                    prev,
                    quote_m_string(new_column),
                    each_body(expression),
                    m_type(*t)
                ),
                None => format!("Table.AddColumn({}, {}, {})", prev, quote_m_string(new_column), each_body(expression)),
            }
        }
        StepKind::AddIndexColumn { new_column, start, step } => format!(
            "Table.AddIndexColumn({}, {}, {}, {}, Int64.Type)",
            prev,
            quote_m_string(new_column),
            start,
            step
        ),
        StepKind::MergeQueries {
            right_query,
            left_keys,
            right_keys,
            join_kind,
            expand,
        } => {
            let jk = match join_kind {
                JoinKind::LeftOuter => "JoinKind.LeftOuter",
                JoinKind::Inner => "JoinKind.Inner",
                JoinKind::RightOuter => "JoinKind.RightOuter",
                JoinKind::FullOuter => "JoinKind.FullOuter",
                JoinKind::LeftAnti => "JoinKind.LeftAnti",
                JoinKind::RightAnti => "JoinKind.RightAnti",
            };
            let cols: Vec<String> = expand.iter().map(|e| e.column.clone()).collect();
            let names: Vec<String> = expand.iter().map(|e| e.as_name.clone()).collect();
            format!(
                "Table.ExpandTableColumn(Table.NestedJoin({}, {}, {}, {}, {}, {}), {}, {}, {})",
                prev,
                m_text_list(left_keys),
                quote_m_ident(right_query),
                m_text_list(right_keys),
                quote_m_string(right_query),
                jk,
                quote_m_string(right_query),
                m_text_list(&cols),
                m_text_list(&names)
            )
        }
        StepKind::AppendQueries { queries } => {
            let mut items = vec![prev.to_string()];
            items.extend(queries.iter().map(|q| quote_m_ident(q)));
            format!("Table.Combine({{{}}})", items.join(", "))
        }
        StepKind::OpaqueM { m } => {
            if m.trim().is_empty() {
                return Err("passo M opaco vazio".into());
            }
            m.trim().to_string()
        }
    })
}

/// Canonical name of the last binding produced by a source (what the first step reads).
pub fn canonical_source_binding(source: &EtlSource) -> String {
    match source.kind {
        SourceKind::Excel => format!("{}_Sheet", source.sheet.as_deref().unwrap_or("Sheet1")),
        SourceKind::ListDates => "Converted to Table".to_string(),
        _ => "Source".to_string(),
    }
}

fn date_parts(iso: &str) -> Result<(i64, i64, i64), String> {
    let parts: Vec<&str> = iso.trim().split('-').collect();
    if parts.len() != 3 {
        return Err(format!("data inválida '{}', esperado AAAA-MM-DD", iso));
    }
    let p = |s: &str| s.parse::<i64>().map_err(|_| format!("data inválida '{}'", iso));
    Ok((p(parts[0])?, p(parts[1])?, p(parts[2])?))
}

/// Source bindings as `(name, expression)`; the last one feeds the first step.
fn source_bindings(source: &EtlSource, avoid: &HashSet<String>) -> Result<Vec<(String, String)>, String> {
    let fresh = |base: &str| -> String {
        if !avoid.contains(base) {
            return base.to_string();
        }
        (1..)
            .map(|i| format!("{}{}", base, i))
            .find(|n| !avoid.contains(n))
            .unwrap()
    };
    let path = || {
        source
            .path
            .as_deref()
            .map(quote_m_string)
            .ok_or_else(|| "fonte sem 'path'".to_string())
    };
    let src_name = fresh("Source");
    Ok(match source.kind {
        SourceKind::Excel => {
            let sheet = source.sheet.as_deref().ok_or("fonte Excel sem 'sheet'")?;
            vec![
                (src_name.clone(), format!("Excel.Workbook(File.Contents({}), null, true)", path()?)),
                (
                    fresh(&canonical_source_binding(source)),
                    format!(
                        "{}{{[Item={},Kind=\"Sheet\"]}}[Data]",
                        quote_m_ident(&src_name),
                        quote_m_string(sheet)
                    ),
                ),
            ]
        }
        SourceKind::Csv => vec![(
            src_name,
            format!(
                "Csv.Document(File.Contents({}),[Encoding=65001, QuoteStyle=QuoteStyle.Csv])",
                path()?
            ),
        )],
        SourceKind::Parquet => vec![(src_name, format!("Parquet.Document(File.Contents({}))", path()?))],
        SourceKind::Json => vec![(
            src_name,
            format!("Table.FromRecords(Json.Document(File.Contents({})))", path()?),
        )],
        SourceKind::Db => {
            let t = source.table.as_deref().ok_or("fonte 'db' sem 'table'")?;
            vec![(src_name, format!("PrismBI.DbTable({})", quote_m_string(t)))]
        }
        SourceKind::Table => {
            let t = source.table.as_deref().ok_or("fonte 'table' sem 'table'")?;
            vec![(src_name, quote_m_ident(t))]
        }
        SourceKind::Inline => {
            let cols = source.columns.clone().unwrap_or_default();
            let rows = source.rows.clone().unwrap_or_default();
            let rows_m = rows
                .iter()
                .map(|r| m_scalar_list(r))
                .collect::<Vec<_>>()
                .join(", ");
            vec![(src_name, format!("#table({}, {{{}}})", m_text_list(&cols), rows_m))]
        }
        SourceKind::ListDates => {
            let (y, mo, d) = date_parts(source.start.as_deref().ok_or("fonte 'listDates' sem 'start'")?)?;
            let count = source.count.ok_or("fonte 'listDates' sem 'count'")?;
            let step = source.step_days.unwrap_or(1);
            let func = source.function.as_deref().unwrap_or("List.Dates");
            vec![
                (
                    src_name.clone(),
                    format!(
                        "{}(#date({}, {}, {}), {}, #duration({}, 0, 0, 0))",
                        if func.contains('.') { func.to_string() } else { quote_m_ident(func) },
                        y,
                        mo,
                        d,
                        count,
                        step
                    ),
                ),
                (
                    fresh(&canonical_source_binding(source)),
                    format!(
                        "Table.FromList({}, Splitter.SplitByNothing(), null, null, ExtraValues.Error)",
                        quote_m_ident(&src_name)
                    ),
                ),
            ]
        }
        SourceKind::OpaqueM => {
            let m = source.m.as_deref().unwrap_or("").trim();
            if m.is_empty() {
                return Err("fonte M opaca vazia".into());
            }
            vec![(src_name, m.to_string())]
        }
    })
}

/// Full `let … in …` script for the query.
pub fn query_to_m(q: &EtlQuery) -> Result<String, String> {
    let mut used: HashSet<String> = HashSet::new();
    let mut step_names = Vec::with_capacity(q.steps.len());
    for s in &q.steps {
        let base = if s.name.trim().is_empty() {
            s.kind.default_step_name().to_string()
        } else {
            s.name.clone()
        };
        let mut name = base.clone();
        let mut i = 2;
        while used.contains(&name) {
            name = format!("{} ({})", base, i);
            i += 1;
        }
        used.insert(name.clone());
        step_names.push(name);
    }
    let mut bindings = source_bindings(&q.source, &used)?;
    let mut prev = quote_m_ident(&bindings.last().unwrap().0);
    for (s, name) in q.steps.iter().zip(&step_names) {
        let expr = m_step_expr(&s.kind, &prev).map_err(|e| format!("passo '{}': {}", name, e))?;
        bindings.push((name.clone(), expr));
        prev = quote_m_ident(name);
    }
    let mut out = String::from("let\n");
    let n = bindings.len();
    for (i, (name, expr)) in bindings.iter().enumerate() {
        out.push_str("    ");
        out.push_str(&quote_m_ident(name));
        out.push_str(" = ");
        // Multi-line opaque/custom text is kept verbatim (no re-indent) so that
        // from_m(to_m(q)) reproduces it byte for byte.
        out.push_str(expr);
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("in\n    ");
    out.push_str(&prev);
    Ok(out)
}
