//! Parses a DAX **query** (`DEFINE ... EVALUATE ...`) into a `QueryProgram`.
//!
//! The query-level grammar (DEFINE/EVALUATE/ORDER BY/START AT) is *not*
//! part of the shared `dax::parser` (that one only parses a single scalar
//! or table expression — a measure/column formula). This module does its
//! own token-level scan to find the boundaries of each define item /
//! EVALUATE statement, then hands each individual sub-expression's source
//! text to the shared `dax::parser::parse_dax`, so every scalar/table
//! function the main engine knows stays available for free and nothing in
//! `dax::{lexer,parser,ast}` (other fronts' shared territory) is touched.
//!
//! Known limitation (documented rather than special-cased): telling a new
//! top-level `DEFINE VAR` item apart from an inline `VAR ... RETURN ...`
//! block *inside* the previous item's expression is ambiguous from tokens
//! alone when that inline block's own body is trivial and immediately
//! followed, with no operator, by another bare `VAR`. This basically never
//! happens in practice (real queries always separate `DEFINE VAR` items by
//! a full expression ending in an operator or a non-VAR tail), so the
//! scanner treats every `VAR` at paren-depth 0 outside an open
//! VAR...RETURN block as a new boundary, which is correct for every case
//! we've found in the DAX Query View's own example snippets (see
//! `tests.rs`).

use crate::dax::ast::Expr;
use crate::dax::lexer::{tokenize, Token, TokenKind};
use crate::dax::parser::parse_dax;
use super::qast::{Define, EvaluateStmt, OrderDir, OrderItem, Pos, QError, QueryProgram, TableExprSrc};

pub fn parse_query(text: &str) -> Result<QueryProgram, QError> {
    let chars: Vec<char> = text.chars().collect();
    let tokens = tokenize(text).map_err(|e| QError::new(e, Pos { line: 1, col: 1 }))?;
    let mut i = 0usize;
    let mut prog = QueryProgram::default();

    loop {
        match tokens.get(i).map(|t| &t.kind) {
            Some(TokenKind::Eof) | None => break,
            _ if is_kw(&tokens[i], "DEFINE") => {
                i += 1;
                i = parse_define_block(&tokens, i, &chars, &mut prog)?;
            }
            _ if is_kw(&tokens[i], "EVALUATE") => {
                let pos = pos_of(&chars, tokens[i].pos);
                i += 1;
                let (stmt, next_i) = parse_evaluate(&tokens, i, &chars, pos)?;
                prog.evaluates.push(stmt);
                i = next_i;
            }
            _ => {
                return Err(QError::new(
                    format!("Esperado DEFINE ou EVALUATE, encontrado '{}'.", describe(&tokens[i].kind)),
                    pos_of(&chars, tokens[i].pos),
                ));
            }
        }
    }

    if prog.evaluates.is_empty() {
        return Err(QError::at_start("A consulta precisa de ao menos um EVALUATE."));
    }
    Ok(prog)
}

fn is_kw(tok: &Token, word: &str) -> bool {
    matches!(&tok.kind, TokenKind::Ident(s) if s.eq_ignore_ascii_case(word))
}

fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(s) => s.clone(),
        other => format!("{:?}", other),
    }
}

pub(super) fn pos_of(chars: &[char], char_idx: usize) -> Pos {
    let mut line = 1usize;
    let mut col = 1usize;
    for &c in chars.iter().take(char_idx.min(chars.len())) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    Pos { line, col }
}

fn text_range(chars: &[char], start: usize, end: usize) -> String {
    let end = end.min(chars.len());
    if start >= end {
        return String::new();
    }
    chars[start..end].iter().collect::<String>()
}

/// End-exclusive token index of the next top-level occurrence of one of
/// `boundary` (case-insensitive bare idents) or EOF, starting at `i`,
/// respecting paren/brace nesting and in-progress `VAR...RETURN` blocks (see
/// module doc).
fn scan_expr_span(tokens: &[Token], mut i: usize, boundary: &[&str]) -> usize {
    let mut depth: i32 = 0;
    let mut expect_return = false;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::Eof => return i,
            TokenKind::LParen | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBrace => depth -= 1,
            TokenKind::Var if depth == 0 => expect_return = true,
            TokenKind::Return if depth == 0 => expect_return = false,
            TokenKind::Comma if depth == 0 && boundary.contains(&",") => return i,
            TokenKind::Ident(s) if depth == 0 && !expect_return => {
                if boundary.iter().any(|b| s.eq_ignore_ascii_case(b)) {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

fn parse_sub_expr(tokens: &[Token], chars: &[char], start: usize, end: usize) -> Result<Expr, QError> {
    let start_pos = pos_of(chars, tokens[start].pos);
    let src = text_range(chars, tokens[start].pos, if end < tokens.len() { tokens[end].pos } else { chars.len() });
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Err(QError::new("Expressão vazia.", start_pos));
    }
    parse_dax(trimmed).map_err(|e| QError::new(e, start_pos))
}

fn parse_table_expr_src(tokens: &[Token], chars: &[char], start: usize, end: usize) -> Result<TableExprSrc, QError> {
    // DATATABLE has brace-literal row data that the shared `dax::parser`
    // grammar doesn't support as a bare expression, so detect and hand-roll
    // it; everything else goes through the shared table/scalar grammar.
    if start < end && is_kw(&tokens[start], "DATATABLE") && matches!(tokens.get(start + 1).map(|t| &t.kind), Some(TokenKind::LParen)) {
        return parse_datatable(tokens, chars, start + 2, end);
    }
    parse_sub_expr(tokens, chars, start, end).map(TableExprSrc::Generic)
}

/// Parses `DATATABLE("Col1", STRING, "Col2", INTEGER, {{v,v},{v,v}})`'s
/// argument list, `tokens[args_start..call_end)` being everything strictly
/// between `DATATABLE(` and the statement/define boundary found by the
/// caller (the matching `)` is located here by paren-depth, not assumed to
/// be `call_end - 1`, so trailing `ORDER BY`/next `DEFINE` text after it is
/// left untouched).
fn parse_datatable(tokens: &[Token], chars: &[char], args_start: usize, call_end: usize) -> Result<TableExprSrc, QError> {
    // Find the matching ')' for the '(' at args_start - 1 within [args_start, call_end).
    let mut depth = 1i32; // we've already consumed the opening '('
    let mut rparen_idx = None;
    let mut k = args_start;
    while k < call_end {
        match &tokens[k].kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    rparen_idx = Some(k);
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    let rparen_idx = rparen_idx.ok_or_else(|| QError::new("DATATABLE: parêntese ')' não encontrado.", pos_of(chars, tokens[args_start.min(tokens.len() - 1)].pos)))?;

    // Walk column defs: String(name), Ident(type), Comma, ... until the
    // outer '{' that starts the row-data literal.
    let mut i = args_start;
    let mut columns: Vec<(String, String)> = Vec::new();
    loop {
        match tokens.get(i).map(|t| &t.kind) {
            Some(TokenKind::LBrace) => break,
            Some(TokenKind::String(name)) => {
                let col_name = name.clone();
                i += 1;
                if matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::Comma)) {
                    i += 1;
                }
                let type_name = match tokens.get(i).map(|t| &t.kind) {
                    Some(TokenKind::Ident(t)) => {
                        let t = t.clone();
                        i += 1;
                        t
                    }
                    other => {
                        return Err(QError::new(
                            format!("DATATABLE: esperado o tipo da coluna '{}', encontrado {:?}.", col_name, other),
                            pos_of(chars, tokens[i.min(tokens.len() - 1)].pos),
                        ))
                    }
                };
                columns.push((col_name, type_name));
                if matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::Comma)) {
                    i += 1;
                }
            }
            other => {
                return Err(QError::new(
                    format!("DATATABLE: esperado nome de coluna entre aspas, encontrado {:?}.", other),
                    pos_of(chars, tokens[i.min(tokens.len() - 1)].pos),
                ))
            }
        }
    }
    if columns.is_empty() {
        return Err(QError::new("DATATABLE: nenhuma coluna declarada.", pos_of(chars, tokens[args_start].pos)));
    }

    // i now points at the outer '{'. Parse `{ {v,v,..} , {v,v,..} , ... }`.
    if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::LBrace)) {
        return Err(QError::new("DATATABLE: esperado '{' para os dados.", pos_of(chars, tokens[i.min(tokens.len() - 1)].pos)));
    }
    i += 1; // consume outer '{'
    let mut rows: Vec<Vec<Expr>> = Vec::new();
    loop {
        match tokens.get(i).map(|t| &t.kind) {
            Some(TokenKind::RBrace) => {
                i += 1;
                break;
            }
            Some(TokenKind::LBrace) => {
                let row_start = i;
                i += 1; // consume row '{'
                let mut row_vals = Vec::new();
                loop {
                    // Scan one value: up to the next Comma or RBrace at depth 0.
                    let val_start = i;
                    let mut d = 0i32;
                    while i < tokens.len() {
                        match &tokens[i].kind {
                            TokenKind::LParen | TokenKind::LBrace => d += 1,
                            TokenKind::RParen => d -= 1,
                            TokenKind::RBrace if d == 0 => break,
                            TokenKind::RBrace => d -= 1,
                            TokenKind::Comma if d == 0 => break,
                            _ => {}
                        }
                        i += 1;
                    }
                    let expr = parse_sub_expr(tokens, chars, val_start, i)?;
                    row_vals.push(expr);
                    match tokens.get(i).map(|t| &t.kind) {
                        Some(TokenKind::Comma) => {
                            i += 1;
                            continue;
                        }
                        Some(TokenKind::RBrace) => {
                            i += 1; // consume row '}'
                            break;
                        }
                        other => {
                            return Err(QError::new(
                                format!("DATATABLE: esperado ',' ou '}}' na linha de dados, encontrado {:?}.", other),
                                pos_of(chars, tokens[i.min(tokens.len() - 1)].pos),
                            ))
                        }
                    }
                }
                if row_vals.len() != columns.len() {
                    return Err(QError::new(
                        format!("DATATABLE: linha com {} valores, esperado {} (uma por coluna).", row_vals.len(), columns.len()),
                        pos_of(chars, tokens[row_start.min(tokens.len() - 1)].pos),
                    ));
                }
                rows.push(row_vals);
                match tokens.get(i).map(|t| &t.kind) {
                    Some(TokenKind::Comma) => {
                        i += 1;
                        continue;
                    }
                    Some(TokenKind::RBrace) => {
                        i += 1;
                        break;
                    }
                    other => {
                        return Err(QError::new(
                            format!("DATATABLE: esperado ',' ou '}}' após uma linha, encontrado {:?}.", other),
                            pos_of(chars, tokens[i.min(tokens.len() - 1)].pos),
                        ))
                    }
                }
            }
            other => {
                return Err(QError::new(
                    format!("DATATABLE: esperado '{{' iniciando uma linha, encontrado {:?}.", other),
                    pos_of(chars, tokens[i.min(tokens.len() - 1)].pos),
                ))
            }
        }
    }
    let _ = rparen_idx; // validated structurally above; kept for clarity/future use.
    let _ = i; // final cursor unused: caller already knows the span's end.
    Ok(TableExprSrc::DataTable { columns, rows })
}

const DEFINE_ITEM_KEYWORDS: [&str; 5] = ["MEASURE", "VAR", "TABLE", "COLUMN", "EVALUATE"];

fn parse_define_block(tokens: &[Token], mut i: usize, chars: &[char], prog: &mut QueryProgram) -> Result<usize, QError> {
    loop {
        match tokens.get(i).map(|t| &t.kind) {
            Some(TokenKind::Var) => {
                i += 1;
                let name = expect_ident(tokens, chars, &mut i, "nome da variável")?;
                expect_eq(tokens, chars, &mut i)?;
                let end = scan_expr_span(tokens, i, &DEFINE_ITEM_KEYWORDS);
                let expr = parse_sub_expr(tokens, chars, i, end)?;
                prog.defines.push(Define::Var { name, expr });
                i = end;
            }
            Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("MEASURE") => {
                i += 1;
                let table = expect_ident(tokens, chars, &mut i, "nome da tabela em MEASURE")?;
                let name = expect_bracket(tokens, chars, &mut i, "nome da medida entre []")?;
                expect_eq(tokens, chars, &mut i)?;
                let end = scan_expr_span(tokens, i, &DEFINE_ITEM_KEYWORDS);
                let expr = parse_sub_expr(tokens, chars, i, end)?;
                prog.defines.push(Define::Measure { table, name, expr });
                i = end;
            }
            Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("TABLE") => {
                i += 1;
                let name = expect_ident(tokens, chars, &mut i, "nome da tabela em TABLE")?;
                expect_eq(tokens, chars, &mut i)?;
                let end = scan_expr_span(tokens, i, &DEFINE_ITEM_KEYWORDS);
                let expr = parse_table_expr_src(tokens, chars, i, end)?;
                prog.defines.push(Define::Table { name, expr });
                i = end;
            }
            Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("COLUMN") => {
                i += 1;
                let table = expect_ident(tokens, chars, &mut i, "nome da tabela em COLUMN")?;
                let name = expect_bracket(tokens, chars, &mut i, "nome da coluna entre []")?;
                expect_eq(tokens, chars, &mut i)?;
                let end = scan_expr_span(tokens, i, &DEFINE_ITEM_KEYWORDS);
                let expr = parse_sub_expr(tokens, chars, i, end)?;
                prog.defines.push(Define::Column { table, name, expr });
                i = end;
            }
            _ => return Ok(i),
        }
    }
}

fn expect_ident(tokens: &[Token], chars: &[char], i: &mut usize, what: &str) -> Result<String, QError> {
    match tokens.get(*i).map(|t| &t.kind) {
        Some(TokenKind::Ident(s)) => {
            let s = s.clone();
            *i += 1;
            Ok(s)
        }
        other => Err(QError::new(format!("Esperado {}, encontrado {:?}.", what, other), pos_of(chars, tokens[(*i).min(tokens.len() - 1)].pos))),
    }
}

fn expect_bracket(tokens: &[Token], chars: &[char], i: &mut usize, what: &str) -> Result<String, QError> {
    match tokens.get(*i).map(|t| &t.kind) {
        Some(TokenKind::Bracket(s)) => {
            let s = s.clone();
            *i += 1;
            Ok(s)
        }
        other => Err(QError::new(format!("Esperado {}, encontrado {:?}.", what, other), pos_of(chars, tokens[(*i).min(tokens.len() - 1)].pos))),
    }
}

fn expect_eq(tokens: &[Token], chars: &[char], i: &mut usize) -> Result<(), QError> {
    match tokens.get(*i).map(|t| &t.kind) {
        Some(TokenKind::Eq) => {
            *i += 1;
            Ok(())
        }
        other => Err(QError::new(format!("Esperado '=', encontrado {:?}.", other), pos_of(chars, tokens[(*i).min(tokens.len() - 1)].pos))),
    }
}

const EVALUATE_BOUNDARY: [&str; 6] = ["ORDER", "START", "DEFINE", "MEASURE", "TABLE", "EVALUATE"];

fn parse_evaluate(tokens: &[Token], i: usize, chars: &[char], pos: Pos) -> Result<(EvaluateStmt, usize), QError> {
    let end = scan_expr_span(tokens, i, &EVALUATE_BOUNDARY);
    let table_expr = parse_table_expr_src(tokens, chars, i, end)?;
    let mut j = end;

    let mut order_by = Vec::new();
    if matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("ORDER")) {
        j += 1;
        match tokens.get(j).map(|t| &t.kind) {
            Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("BY") => j += 1,
            other => return Err(QError::new(format!("Esperado BY após ORDER, encontrado {:?}.", other), pos_of(chars, tokens[j.min(tokens.len() - 1)].pos))),
        }
        loop {
            let item_end = scan_expr_span(tokens, j, &["ASC", "DESC", ",", "START", "DEFINE", "MEASURE", "TABLE", "EVALUATE"]);
            let expr = parse_sub_expr(tokens, chars, j, item_end)?;
            j = item_end;
            let dir = match tokens.get(j).map(|t| &t.kind) {
                Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("ASC") => {
                    j += 1;
                    OrderDir::Asc
                }
                Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("DESC") => {
                    j += 1;
                    OrderDir::Desc
                }
                _ => OrderDir::Asc,
            };
            order_by.push(OrderItem { expr, dir });
            if matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::Comma)) {
                j += 1;
                continue;
            }
            break;
        }
    }

    let mut start_at = Vec::new();
    if matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("START")) {
        j += 1;
        match tokens.get(j).map(|t| &t.kind) {
            Some(TokenKind::Ident(s)) if s.eq_ignore_ascii_case("AT") => j += 1,
            other => return Err(QError::new(format!("Esperado AT após START, encontrado {:?}.", other), pos_of(chars, tokens[j.min(tokens.len() - 1)].pos))),
        }
        loop {
            let item_end = scan_expr_span(tokens, j, &[",", "DEFINE", "MEASURE", "TABLE", "EVALUATE"]);
            let expr = parse_sub_expr(tokens, chars, j, item_end)?;
            start_at.push(expr);
            j = item_end;
            if matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::Comma)) {
                j += 1;
                continue;
            }
            break;
        }
    }

    Ok((EvaluateStmt { table_expr, order_by, start_at, pos }, j))
}
