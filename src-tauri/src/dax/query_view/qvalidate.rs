//! `dax_validate`: syntax + light semantic diagnostics for a single DAX
//! expression (measure / calculated column / calculated table / visual
//! calc), with position info derived two ways:
//! - **Syntax errors**: the shared `dax::lexer`/`dax::parser` don't carry
//!   spans, so on a parse failure we re-scan the token stream ourselves to
//!   find the first structural problem (unmatched `(`/`[`/`{`, a trailing
//!   token after what should've been the end of the expression) and point
//!   at that token; otherwise we fall back to the error text's own
//!   `"posição N"` suffix (the lexer includes one for unexpected
//!   characters) or, failing that, the start of the expression.
//! - **Semantic errors** ("função desconhecida", "coluna/tabela
//!   inexistente"): detected by actually *running* the expression through
//!   the shared evaluator (reusing its existing, already-PT-BR error
//!   messages — "Tabela 'X' não encontrada", "Coluna 'X[Y]' não
//!   encontrada", "Função DAX 'X' ainda não implementada...", "Referência
//!   '[X]' não encontrada.") and then locating the offending name's first
//!   token occurrence to get a real line/column, instead of duplicating a
//!   function/column whitelist that would drift from the real one.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::dax::ast::Expr;
use crate::dax::evaluator::{evaluate_ast, evaluate_table_ast, RowContext};
use crate::dax::lexer::{tokenize, Token, TokenKind};
use crate::dax::parser::parse_dax;
use crate::dax::{DaxModel, DaxValue, FilterContext};

use super::qast::Pos;
use super::qparser::pos_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidateContext {
    Measure,
    CalculatedColumn,
    CalculatedTable,
    VisualCalc,
}

impl ValidateContext {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "measure" => Some(Self::Measure),
            "calculatedColumn" => Some(Self::CalculatedColumn),
            "calculatedTable" => Some(Self::CalculatedTable),
            "visualCalc" => Some(Self::VisualCalc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationDiagnostic {
    pub severity: &'static str, // "error" | "warning"
    pub message: String,
    pub line: usize,
    pub column: usize,
    /// Characters to underline from (line,column); best-effort, 1 when unknown.
    pub length: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub ok: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub result_type: Option<&'static str>,
    pub sample_value: Option<JsonValue>,
}

fn diag(severity: &'static str, message: String, pos: Pos, length: usize) -> ValidationDiagnostic {
    ValidationDiagnostic { severity, message, line: pos.line.max(1), column: pos.col.max(1), length: length.max(1) }
}

/// Finds the first structural problem in `tokens` (unmatched bracket/paren
/// or a stray token after a complete-looking expression) to give a parse
/// error a real position when the shared parser's error string carries
/// none.
fn locate_structural_issue(tokens: &[Token], chars: &[char]) -> Pos {
    let mut stack: Vec<(char, usize)> = Vec::new();
    for t in tokens {
        match &t.kind {
            TokenKind::LParen => stack.push(('(', t.pos)),
            TokenKind::LBrace => stack.push(('{', t.pos)),
            TokenKind::RParen => {
                if stack.pop().is_none() {
                    return pos_of(chars, t.pos);
                }
            }
            TokenKind::RBrace => {
                if stack.pop().is_none() {
                    return pos_of(chars, t.pos);
                }
            }
            TokenKind::Eof => break,
            _ => {}
        }
    }
    if let Some((_, p)) = stack.first() {
        return pos_of(chars, *p);
    }
    Pos { line: 1, col: 1 }
}

/// Extracts a numeric "posição N" suffix from a lexer error message, if present.
fn pos_from_lexer_message(msg: &str, chars: &[char]) -> Option<Pos> {
    let marker = "posição ";
    let idx = msg.find(marker)?;
    let digits: String = msg[idx + marker.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: usize = digits.parse().ok()?;
    Some(pos_of(chars, n))
}

fn find_ident_pos(tokens: &[Token], chars: &[char], name: &str, skip: usize) -> Option<Pos> {
    let mut count = 0usize;
    for t in tokens {
        if let TokenKind::Ident(s) = &t.kind {
            if s.eq_ignore_ascii_case(name) {
                if count == skip {
                    return Some(pos_of(chars, t.pos));
                }
                count += 1;
            }
        }
    }
    None
}

fn find_bracket_pos(tokens: &[Token], chars: &[char], name: &str) -> Option<Pos> {
    tokens.iter().find(|t| matches!(&t.kind, TokenKind::Bracket(s) if s.eq_ignore_ascii_case(name))).map(|t| pos_of(chars, t.pos))
}

fn find_tablecol_pos(tokens: &[Token], chars: &[char], table: &str, col: &str) -> Option<Pos> {
    for i in 0..tokens.len().saturating_sub(1) {
        if let (TokenKind::Ident(t), TokenKind::Bracket(c)) = (&tokens[i].kind, &tokens[i + 1].kind) {
            if t.eq_ignore_ascii_case(table) && c.eq_ignore_ascii_case(col) {
                return Some(pos_of(chars, tokens[i].pos));
            }
        }
    }
    None
}

fn first_quoted(msg: &str) -> Option<String> {
    let start = msg.find('\'')?;
    let rest = &msg[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Best-effort position + length for an evaluator runtime-error message
/// (see module doc for the message shapes this recognizes).
fn locate_runtime_error(tokens: &[Token], chars: &[char], msg: &str) -> (Pos, usize) {
    let Some(q) = first_quoted(msg) else { return (Pos { line: 1, col: 1 }, 1) };
    if let Some(name) = q.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        if let Some(p) = find_bracket_pos(tokens, chars, name) {
            return (p, name.chars().count() + 2);
        }
    } else if let Some((t, rest)) = q.split_once('[') {
        let c = rest.trim_end_matches(']');
        if let Some(p) = find_tablecol_pos(tokens, chars, t, c) {
            return (p, t.chars().count() + c.chars().count() + 3);
        }
    } else if let Some(p) = find_ident_pos(tokens, chars, &q, 0) {
        return (p, q.chars().count());
    }
    (Pos { line: 1, col: 1 }, 1)
}

/// `(function name, min args, max args — None = unbounded)`. Small,
/// hand-picked subset of DAX functions with an unambiguous arity, checked
/// purely syntactically (no evaluation needed) so a wrong-arity call is
/// flagged even when it would otherwise silently mis-evaluate. Not
/// exhaustive — see the final report.
const ARITY: &[(&str, usize, Option<usize>)] = &[
    ("DIVIDE", 2, Some(3)),
    ("RELATED", 1, Some(1)),
    ("RELATEDTABLE", 1, Some(1)),
    ("IF", 2, Some(3)),
    ("SUM", 1, Some(1)),
    ("SUMX", 2, Some(2)),
    ("AVERAGEX", 2, Some(2)),
    ("COUNTX", 2, Some(2)),
    ("MINX", 2, Some(2)),
    ("MAXX", 2, Some(2)),
    ("FILTER", 2, Some(2)),
    ("TOPN", 2, Some(4)),
    ("VALUES", 1, Some(1)),
    ("DISTINCT", 1, Some(1)),
    ("RANKX", 2, Some(5)),
    ("USERELATIONSHIP", 2, Some(2)),
    ("CROSSFILTER", 3, Some(3)),
    ("ROUND", 2, Some(2)),
    ("ROUNDUP", 2, Some(2)),
    ("ROUNDDOWN", 2, Some(2)),
    ("CONCATENATE", 2, Some(2)),
    ("DATE", 3, Some(3)),
    ("EDATE", 2, Some(2)),
    ("SELECTEDVALUE", 1, Some(2)),
    ("LOOKUPVALUE", 3, None),
];

fn check_arity(expr: &Expr, tokens: &[Token], chars: &[char], seen: &mut HashMap<String, usize>, out: &mut Vec<ValidationDiagnostic>) {
    match expr {
        Expr::Call { name, args } => {
            let upper = name.to_uppercase();
            if let Some((_, min, max)) = ARITY.iter().find(|(n, _, _)| *n == upper) {
                if args.len() < *min || max.is_some_and(|m| args.len() > m) {
                    let skip = *seen.get(&upper).unwrap_or(&0);
                    let pos = find_ident_pos(tokens, chars, name, skip).unwrap_or(Pos { line: 1, col: 1 });
                    let expected = match max {
                        Some(m) if m == min => format!("{}", min),
                        Some(m) => format!("entre {} e {}", min, m),
                        None => format!("ao menos {}", min),
                    };
                    out.push(diag(
                        "error",
                        format!("{}: espera {} argumento(s), recebeu {}.", name, expected, args.len()),
                        pos,
                        name.chars().count(),
                    ));
                }
            }
            *seen.entry(upper).or_insert(0) += 1;
            for a in args {
                check_arity(a, tokens, chars, seen, out);
            }
        }
        Expr::Unary { expr, .. } => check_arity(expr, tokens, chars, seen, out),
        Expr::Binary { left, right, .. } => {
            check_arity(left, tokens, chars, seen, out);
            check_arity(right, tokens, chars, seen, out);
        }
        Expr::InList { expr, list } => {
            check_arity(expr, tokens, chars, seen, out);
            for e in list {
                check_arity(e, tokens, chars, seen, out);
            }
        }
        Expr::VarBlock { bindings, body } => {
            for (_, e) in bindings {
                check_arity(e, tokens, chars, seen, out);
            }
            check_arity(body, tokens, chars, seen, out);
        }
        _ => {}
    }
}

fn collect_bracket_refs(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::BracketRef(name) => out.push(name.clone()),
        Expr::Unary { expr, .. } => collect_bracket_refs(expr, out),
        Expr::Binary { left, right, .. } => {
            collect_bracket_refs(left, out);
            collect_bracket_refs(right, out);
        }
        Expr::InList { expr, list } => {
            collect_bracket_refs(expr, out);
            for e in list {
                collect_bracket_refs(e, out);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_bracket_refs(a, out);
            }
        }
        Expr::VarBlock { bindings, body } => {
            for (_, e) in bindings {
                collect_bracket_refs(e, out);
            }
            collect_bracket_refs(body, out);
        }
        _ => {}
    }
}

/// DFS over the model's own measure-formula graph starting at `name`,
/// looking for a cycle. Returns the cycle's member names when found.
fn measure_cycle_dfs(model: &DaxModel, name: &str, path: &mut Vec<String>, visiting: &mut HashSet<String>) -> Option<Vec<String>> {
    let key = name.to_lowercase();
    if visiting.contains(&key) {
        let mut cycle = path.clone();
        cycle.push(name.to_string());
        return Some(cycle);
    }
    let Some(m) = model.get_measure(name) else { return None };
    let Ok(ast) = parse_dax(&m.formula) else { return None };
    visiting.insert(key.clone());
    path.push(name.to_string());
    let mut refs = Vec::new();
    collect_bracket_refs(&ast, &mut refs);
    for r in refs {
        if let Some(cycle) = measure_cycle_dfs(model, &r, path, visiting) {
            return Some(cycle);
        }
    }
    path.pop();
    visiting.remove(&key);
    None
}

fn type_name(v: &DaxValue) -> &'static str {
    match v {
        DaxValue::Blank => "blank",
        DaxValue::Boolean(_) => "boolean",
        DaxValue::Integer(_) => "integer",
        DaxValue::Decimal(_) => "decimal",
        DaxValue::Currency(_) => "currency",
        DaxValue::DateTime(_) => "date",
        DaxValue::Text(_) => "text",
    }
}

pub fn validate_expression(model: &DaxModel, expression: &str, context: ValidateContext, table: Option<&str>) -> ValidationResult {
    let chars: Vec<char> = expression.chars().collect();
    let tokens = match tokenize(expression) {
        Ok(t) => t,
        Err(e) => {
            let pos = pos_from_lexer_message(&e, &chars).unwrap_or(Pos { line: 1, col: 1 });
            return ValidationResult { ok: false, diagnostics: vec![diag("error", e, pos, 1)], result_type: None, sample_value: None };
        }
    };

    let ast = match parse_dax(expression) {
        Ok(a) => a,
        Err(e) => {
            let pos = pos_from_lexer_message(&e, &chars).unwrap_or_else(|| locate_structural_issue(&tokens, &chars));
            return ValidationResult { ok: false, diagnostics: vec![diag("error", e, pos, 1)], result_type: None, sample_value: None };
        }
    };

    let mut diagnostics = Vec::new();
    check_arity(&ast, &tokens, &chars, &mut HashMap::new(), &mut diagnostics);

    let mut direct_refs = Vec::new();
    collect_bracket_refs(&ast, &mut direct_refs);
    for r in &direct_refs {
        if let Some(cycle) = measure_cycle_dfs(model, r, &mut vec![], &mut HashSet::new()) {
            diagnostics.push(diag(
                "error",
                format!("Referência circular entre medidas: {}.", cycle.join(" -> ")),
                find_bracket_pos(&tokens, &chars, r).unwrap_or(Pos { line: 1, col: 1 }),
                r.chars().count() + 2,
            ));
        }
    }

    if !diagnostics.is_empty() {
        return ValidationResult { ok: false, diagnostics, result_type: None, sample_value: None };
    }

    let ctx = FilterContext::default();
    match context {
        ValidateContext::CalculatedTable => match evaluate_table_ast("Amostra", &ast, model, &ctx) {
            Ok(tbl) => {
                let sample_rows: Vec<JsonValue> = tbl
                    .rows
                    .iter()
                    .take(5)
                    .map(|r| {
                        let mut obj = serde_json::Map::new();
                        for (i, c) in tbl.columns.iter().enumerate() {
                            obj.insert(c.clone(), r[i].to_json());
                        }
                        JsonValue::Object(obj)
                    })
                    .collect();
                ValidationResult {
                    ok: true,
                    diagnostics: vec![],
                    result_type: Some("table"),
                    sample_value: Some(JsonValue::Array(sample_rows)),
                }
            }
            Err(e) => {
                let (pos, len) = locate_runtime_error(&tokens, &chars, &e);
                ValidationResult { ok: false, diagnostics: vec![diag("error", e, pos, len)], result_type: None, sample_value: None }
            }
        },
        ValidateContext::CalculatedColumn => {
            let tbl = table.and_then(|t| model.get_table(t));
            let result = match tbl {
                Some(t) if !t.rows.is_empty() => evaluate_ast(&ast, model, &ctx, &[RowContext { table: t, row_idx: 0 }], &HashMap::new()),
                _ => evaluate_ast(&ast, model, &ctx, &[], &HashMap::new()),
            };
            match result {
                Ok(v) => ValidationResult { ok: true, diagnostics: vec![], result_type: Some(type_name(&v)), sample_value: Some(v.to_json()) },
                Err(e) => {
                    let (pos, len) = locate_runtime_error(&tokens, &chars, &e);
                    ValidationResult { ok: false, diagnostics: vec![diag("error", e, pos, len)], result_type: None, sample_value: None }
                }
            }
        }
        ValidateContext::Measure | ValidateContext::VisualCalc => match evaluate_ast(&ast, model, &ctx, &[], &HashMap::new()) {
            Ok(v) => ValidationResult { ok: true, diagnostics: vec![], result_type: Some(type_name(&v)), sample_value: Some(v.to_json()) },
            Err(e) => {
                let (pos, len) = locate_runtime_error(&tokens, &chars, &e);
                ValidationResult { ok: false, diagnostics: vec![diag("error", e, pos, len)], result_type: None, sample_value: None }
            }
        },
    }
}
