//! `addCustomColumn` formulas: the M subset from `docs/ETL_V09.md` compiled to
//! a DuckDB SQL expression.
//!
//! Month/day names come from the static tables below, never from the OS/ICU
//! locale, so `Date.MonthName([Data], "pt-BR")` is identical on every machine.

use super::m_parse::{parse_expression, BinOp, Expr, ExprKind, UnOp};
use super::sql::{
    cat_of, clean_sql, convert_expr, culture_info, lit_num, lit_str, proper_sql, quote_ident, to_text, trim_ws_sql,
    Cat, Chain,
};
use crate::etl::model::EtlType;

pub const MONTHS_PT: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro",
    "novembro", "dezembro",
];
/// Sunday first (index = `Date.DayOfWeek(.., Day.Sunday)`).
pub const DAYS_PT: [&str; 7] = [
    "domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado",
];
pub const MONTHS_EN: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];
pub const DAYS_EN: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
pub const MONTHS_ES: [&str; 12] = [
    "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre",
    "diciembre",
];
pub const DAYS_ES: [&str; 7] = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];

fn name_tables(culture: Option<&str>) -> Result<(&'static [&'static str; 12], &'static [&'static str; 7]), String> {
    let lang = culture
        .map(|c| c.trim().to_lowercase())
        .unwrap_or_else(|| "en".into());
    let lang = lang.split(['-', '_']).next().unwrap_or("").to_string();
    match lang.as_str() {
        "pt" => Ok((&MONTHS_PT, &DAYS_PT)),
        "en" | "" => Ok((&MONTHS_EN, &DAYS_EN)),
        "es" => Ok((&MONTHS_ES, &DAYS_ES)),
        _ => Err(format!(
            "nomes de mês/dia para a cultura '{}' não estão disponíveis (suportadas: pt, en, es)",
            culture.unwrap_or("")
        )),
    }
}

fn list_lit(items: &[&str]) -> String {
    format!("[{}]", items.iter().map(|s| lit_str(s)).collect::<Vec<_>>().join(", "))
}

struct Tr<'a> {
    chain: &'a Chain,
    /// `_` binding when translating inside `Table.TransformColumns` (the
    /// cell's SQL expression and category). `None` in `addCustomColumn`.
    underscore: Option<(String, Cat)>,
}

type R = Result<(String, Cat), String>;

fn unsupported(what: &str) -> String {
    format!(
        "'{}' não é suportado em colunas personalizadas (subconjunto: [Coluna], literais, + - * / &, comparações, and/or/not, if/then/else e funções Text./Number./Date. documentadas)",
        what
    )
}

fn first_day_index(e: Option<&Expr>) -> Result<i64, String> {
    let Some(e) = e else { return Ok(0) };
    let days = [
        "Day.Sunday",
        "Day.Monday",
        "Day.Tuesday",
        "Day.Wednesday",
        "Day.Thursday",
        "Day.Friday",
        "Day.Saturday",
    ];
    if let Some(n) = e.unparen().as_ident() {
        if let Some(i) = days.iter().position(|d| *d == n) {
            return Ok(i as i64);
        }
    }
    if let Some(n) = e.as_number() {
        if (0.0..=6.0).contains(&n) && n.fract() == 0.0 {
            return Ok(n as i64);
        }
    }
    Err("primeiro dia da semana inválido".into())
}

impl<'a> Tr<'a> {
    fn culture_arg(&self, e: Option<&Expr>) -> Result<Option<String>, String> {
        match e {
            None => Ok(None),
            Some(x) if x.is_null() => Ok(None),
            Some(x) => x
                .unparen()
                .as_text()
                .map(|s| Some(s.to_string()))
                .ok_or_else(|| "a cultura deve ser um texto literal, ex. \"pt-BR\"".into()),
        }
    }

    fn date_arg(&self, e: &Expr) -> R {
        let (s, c) = self.tr(e)?;
        match c {
            Cat::Date | Cat::Timestamp | Cat::Null | Cat::Other => Ok((s, c)),
            Cat::Text => Ok((convert_expr(&s, c, EtlType::Date, culture_info(None)?, false), Cat::Date)),
            _ => Err("a função de data exige um valor de data".into()),
        }
    }

    fn text_arg(&self, e: &Expr) -> Result<String, String> {
        let (s, c) = self.tr(e)?;
        Ok(match c {
            Cat::Text | Cat::Null | Cat::Other => s,
            other => to_text(&s, other, culture_info(None)?),
        })
    }

    fn num_arg(&self, e: &Expr) -> Result<String, String> {
        let (s, c) = self.tr(e)?;
        if matches!(c, Cat::Text | Cat::Date | Cat::Timestamp | Cat::Bool) {
            return Err("operação numérica sobre valor não numérico".into());
        }
        Ok(s)
    }

    fn tr(&self, e: &Expr) -> R {
        match &e.kind {
            ExprKind::Null => Ok(("NULL".into(), Cat::Null)),
            ExprKind::Bool(b) => Ok((if *b { "TRUE" } else { "FALSE" }.into(), Cat::Bool)),
            ExprKind::Num(n) => {
                let cat = if n.fract() == 0.0 && n.abs() < 1e15 { Cat::Int } else { Cat::Float };
                Ok((lit_num(*n)?, cat))
            }
            ExprKind::Text(t) => Ok((lit_str(t), Cat::Text)),
            ExprKind::Paren(x) => {
                let (s, c) = self.tr(x)?;
                Ok((format!("({})", s), c))
            }
            ExprKind::Field {
                target: None,
                name,
                optional,
            } => match self.chain.col(name) {
                Some(info) if !super::sql::is_hidden(name) => Ok((quote_ident(name), cat_of(&info.ty))),
                _ if *optional => Ok(("NULL".into(), Cat::Null)),
                _ => Err(format!("A coluna '{}' da tabela não foi encontrada.", name)),
            },
            ExprKind::Unary { op, expr } => {
                let (s, c) = self.tr(expr)?;
                match op {
                    UnOp::Not => Ok((format!("(NOT {})", s), Cat::Bool)),
                    UnOp::Neg => Ok((format!("(-{})", s), c)),
                    UnOp::Plus => Ok((s, c)),
                }
            }
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right),
            ExprKind::If { cond, then, els } => {
                let (cs, _) = self.tr(cond)?;
                let (ts, tc) = self.tr(then)?;
                let (es, ec) = self.tr(els)?;
                let cat = match (tc, ec) {
                    (a, Cat::Null) => a,
                    (Cat::Null, b) => b,
                    (a, b) if a == b => a,
                    (a, b) if a.is_numeric() && b.is_numeric() => Cat::Float,
                    _ => Cat::Other,
                };
                Ok((format!("(CASE WHEN {} THEN {} ELSE {} END)", cs, ts, es), cat))
            }
            ExprKind::Call { .. } => self.call(e),
            ExprKind::Each(body) => self.tr(body),
            ExprKind::Ident(n) if n == "_" => match &self.underscore {
                Some((s, c)) => Ok((s.clone(), *c)),
                None => Err(unsupported("_ (registro da linha)")),
            },
            ExprKind::Ident(n) => Err(unsupported(n)),
            ExprKind::Let { .. } => Err(unsupported("let")),
            ExprKind::Func { .. } => Err(unsupported("função")),
            ExprKind::Record(_) => Err(unsupported("registro")),
            ExprKind::List(_) | ExprKind::Range(..) => Err(unsupported("lista")),
            ExprKind::Field { .. } | ExprKind::Item { .. } => Err(unsupported("acesso a campo/item")),
            ExprKind::Type(_) => Err(unsupported("type")),
            ExprKind::Try { .. } => Err(unsupported("try")),
            ExprKind::Error(_) => Err(unsupported("error")),
            ExprKind::NotImplemented | ExprKind::Opaque(_) => Err(unsupported("...")),
        }
    }

    fn binary(&self, op: BinOp, left: &Expr, right: &Expr) -> R {
        match op {
            BinOp::And | BinOp::Or => {
                let (l, _) = self.tr(left)?;
                let (r, _) = self.tr(right)?;
                let sym = if op == BinOp::And { "AND" } else { "OR" };
                Ok((format!("({} {} {})", l, sym, r), Cat::Bool))
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let (l, _) = self.tr(left)?;
                let (r, _) = self.tr(right)?;
                let sym = match op {
                    // M: null = null is true, x = null is false.
                    BinOp::Eq => "IS NOT DISTINCT FROM",
                    BinOp::Ne => "IS DISTINCT FROM",
                    BinOp::Lt => "<",
                    BinOp::Le => "<=",
                    BinOp::Gt => ">",
                    _ => ">=",
                };
                Ok((format!("({} {} {})", l, sym, r), Cat::Bool))
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                let (l, lc) = self.tr(left)?;
                let (r, rc) = self.tr(right)?;
                for c in [lc, rc] {
                    if matches!(c, Cat::Text | Cat::Date | Cat::Timestamp | Cat::Bool) {
                        return Err("operadores aritméticos exigem números (use Date.AddDays para datas)".into());
                    }
                }
                let sym = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    _ => "/",
                };
                let cat = if op == BinOp::Div {
                    Cat::Float
                } else if lc == Cat::Int && rc == Cat::Int {
                    Cat::Int
                } else if lc == Cat::Null || rc == Cat::Null {
                    Cat::Null
                } else {
                    Cat::Float
                };
                let expr = if op == BinOp::Div {
                    format!("(CAST({} AS DOUBLE) / {})", l, r)
                } else {
                    format!("({} {} {})", l, sym, r)
                };
                Ok((expr, cat))
            }
            BinOp::Concat => {
                let (l, lc) = self.tr(left)?;
                let (r, rc) = self.tr(right)?;
                for c in [lc, rc] {
                    if !matches!(c, Cat::Text | Cat::Null | Cat::Other) {
                        return Err("o operador & exige texto (use Text.From)".into());
                    }
                }
                Ok((format!("({} || {})", l, r), Cat::Text))
            }
            BinOp::Coalesce => {
                let (l, lc) = self.tr(left)?;
                let (r, rc) = self.tr(right)?;
                Ok((format!("COALESCE({}, {})", l, r), if lc == Cat::Null { rc } else { lc }))
            }
            BinOp::Meta | BinOp::Is | BinOp::As => Err(unsupported("meta/is/as")),
        }
    }

    fn call(&self, e: &Expr) -> R {
        let Some((f, args)) = e.as_call() else {
            return Err(unsupported("chamada"));
        };
        let n = args.len();
        let arity = |min: usize, max: usize| -> Result<(), String> {
            if n < min || n > max {
                Err(format!("{} espera {}..{} argumentos, recebeu {}", f, min, max, n))
            } else {
                Ok(())
            }
        };
        match f {
            "Text.Upper" | "Text.Lower" => {
                arity(1, 2)?;
                let t = self.text_arg(&args[0])?;
                let fun = if f == "Text.Upper" { "upper" } else { "lower" };
                Ok((format!("{}({})", fun, t), Cat::Text))
            }
            "Text.Proper" => {
                arity(1, 2)?;
                Ok((proper_sql(&self.text_arg(&args[0])?), Cat::Text))
            }
            "Text.Trim" => {
                arity(1, 1)?;
                Ok((trim_ws_sql(&self.text_arg(&args[0])?), Cat::Text))
            }
            "Text.Start" | "Text.End" => {
                arity(2, 2)?;
                let t = self.text_arg(&args[0])?;
                let k = self.num_arg(&args[1])?;
                let fun = if f == "Text.Start" { "left" } else { "right" };
                Ok((format!("{}({}, CAST({} AS BIGINT))", fun, t, k), Cat::Text))
            }
            "Text.Middle" => {
                arity(2, 3)?;
                let t = self.text_arg(&args[0])?;
                let s = self.num_arg(&args[1])?;
                Ok(match args.get(2) {
                    Some(c) => (
                        format!(
                            "substr({}, CAST({} AS BIGINT) + 1, CAST({} AS BIGINT))",
                            t,
                            s,
                            self.num_arg(c)?
                        ),
                        Cat::Text,
                    ),
                    None => (format!("substr({}, CAST({} AS BIGINT) + 1)", t, s), Cat::Text),
                })
            }
            "Text.Length" => {
                arity(1, 1)?;
                Ok((format!("length({})", self.text_arg(&args[0])?), Cat::Int))
            }
            "Text.Contains" => {
                arity(2, 2)?;
                Ok((
                    format!("contains({}, {})", self.text_arg(&args[0])?, self.text_arg(&args[1])?),
                    Cat::Bool,
                ))
            }
            "Text.From" => {
                arity(1, 2)?;
                let (s, c) = self.tr(&args[0])?;
                let cul = culture_info(self.culture_arg(args.get(1))?.as_deref())?;
                Ok((to_text(&s, c, cul), Cat::Text))
            }
            "Number.Round" => {
                arity(1, 3)?;
                let x = self.num_arg(&args[0])?;
                let d = match args.get(1) {
                    Some(d) if !d.is_null() => self.num_arg(d)?,
                    _ => "0".into(),
                };
                let fun = match args.get(2).and_then(|m| m.unparen().as_ident()) {
                    None | Some("RoundingMode.ToEven") => "round_even",
                    Some("RoundingMode.AwayFromZero") => "round",
                    Some(other) => return Err(unsupported(other)),
                };
                Ok((format!("{}(CAST({} AS DOUBLE), CAST({} AS INTEGER))", fun, x, d), Cat::Float))
            }
            "Number.Abs" => {
                arity(1, 1)?;
                let (s, c) = self.tr(&args[0])?;
                if !(c.is_numeric() || c == Cat::Null || c == Cat::Other) {
                    return Err("Number.Abs exige um número".into());
                }
                Ok((format!("abs({})", s), c))
            }
            "Number.From" => {
                arity(1, 2)?;
                let (s, c) = self.tr(&args[0])?;
                let cul = culture_info(self.culture_arg(args.get(1))?.as_deref())?;
                Ok((convert_expr(&s, c, EtlType::Decimal, cul, false), Cat::Float))
            }
            "Date.From" => {
                arity(1, 2)?;
                let (s, c) = self.tr(&args[0])?;
                let cul = culture_info(self.culture_arg(args.get(1))?.as_deref())?;
                Ok((convert_expr(&s, c, EtlType::Date, cul, false), Cat::Date))
            }
            "Date.Year" | "Date.Month" | "Date.Day" | "Date.Quarter" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                let fun = match f {
                    "Date.Year" => "year",
                    "Date.Month" => "month",
                    "Date.Day" => "day",
                    _ => "quarter",
                };
                Ok((format!("CAST({}({}) AS BIGINT)", fun, d), Cat::Int))
            }
            "Date.DayOfWeek" => {
                arity(1, 2)?;
                let (d, _) = self.date_arg(&args[0])?;
                let k = first_day_index(args.get(1))?;
                Ok((format!("CAST(((date_part('dow', {}) - {} + 7) % 7) AS BIGINT)", d, k), Cat::Int))
            }
            "Date.WeekOfMonth" => {
                arity(1, 2)?;
                let (d, _) = self.date_arg(&args[0])?;
                let k = first_day_index(args.get(1))?;
                // Week 1 contains the 1st; weeks start on `k` (Sunday by default).
                Ok((
                    format!(
                        "CAST((day({d}) - 1 + ((date_part('dow', date_trunc('month', {d})) - {k} + 7) % 7)) // 7 + 1 AS BIGINT)",
                        d = d,
                        k = k
                    ),
                    Cat::Int,
                ))
            }
            "Date.MonthName" | "Date.DayOfWeekName" => {
                arity(1, 2)?;
                let (d, _) = self.date_arg(&args[0])?;
                let cul = self.culture_arg(args.get(1))?;
                let (months, days) = name_tables(cul.as_deref())?;
                Ok(if f == "Date.MonthName" {
                    (format!("list_extract({}, month({}))", list_lit(months), d), Cat::Text)
                } else {
                    (format!("list_extract({}, date_part('dow', {}) + 1)", list_lit(days), d), Cat::Text)
                })
            }
            "Date.AddDays" | "Date.AddMonths" => {
                arity(2, 2)?;
                let (d, c) = self.date_arg(&args[0])?;
                let k = self.num_arg(&args[1])?;
                let unit = if f == "Date.AddDays" { "to_days" } else { "to_months" };
                let sum = format!("({} + {}(CAST({} AS INTEGER)))", d, unit, k);
                Ok(if c == Cat::Timestamp {
                    (sum, Cat::Timestamp)
                } else {
                    (format!("CAST({} AS DATE)", sum), Cat::Date)
                })
            }
            "#date" => {
                arity(3, 3)?;
                Ok((
                    format!(
                        "make_date(CAST({} AS BIGINT), CAST({} AS BIGINT), CAST({} AS BIGINT))",
                        self.num_arg(&args[0])?,
                        self.num_arg(&args[1])?,
                        self.num_arg(&args[2])?
                    ),
                    Cat::Date,
                ))
            }
            // ---- Rodada 2: texto ----
            "Text.BeforeDelimiter" => {
                arity(2, 2)?;
                let t = self.text_arg(&args[0])?;
                let d = self.text_arg(&args[1])?;
                Ok((
                    format!(
                        "(CASE WHEN strpos({t}, {d}) > 0 THEN substr({t}, 1, strpos({t}, {d}) - 1) END)",
                        t = t,
                        d = d
                    ),
                    Cat::Text,
                ))
            }
            "Text.AfterDelimiter" => {
                arity(2, 2)?;
                let t = self.text_arg(&args[0])?;
                let d = self.text_arg(&args[1])?;
                Ok((
                    format!(
                        "(CASE WHEN strpos({t}, {d}) > 0 THEN substr({t}, strpos({t}, {d}) + length({d})) END)",
                        t = t,
                        d = d
                    ),
                    Cat::Text,
                ))
            }
            "Text.BetweenDelimiters" => {
                arity(3, 3)?;
                let t = self.text_arg(&args[0])?;
                let s = self.text_arg(&args[1])?;
                let e = self.text_arg(&args[2])?;
                Ok((
                    format!(
                        "(CASE WHEN strpos({t}, {s}) > 0 AND strpos(substr({t}, strpos({t}, {s}) + length({s})), {e}) > 0 \
                         THEN substr({t}, strpos({t}, {s}) + length({s}), strpos(substr({t}, strpos({t}, {s}) + length({s})), {e}) - 1) END)",
                        t = t,
                        s = s,
                        e = e
                    ),
                    Cat::Text,
                ))
            }
            "Text.PadStart" | "Text.PadEnd" => {
                arity(2, 3)?;
                let t = self.text_arg(&args[0])?;
                let k = self.num_arg(&args[1])?;
                let c = match args.get(2) {
                    Some(c) => self.text_arg(c)?,
                    None => lit_str(" "),
                };
                let fun = if f == "Text.PadStart" { "lpad" } else { "rpad" };
                Ok((format!("{}({}, CAST({} AS INTEGER), {})", fun, t, k, c), Cat::Text))
            }
            "Text.Replace" => {
                arity(3, 3)?;
                let t = self.text_arg(&args[0])?;
                let old = self.text_arg(&args[1])?;
                let new = self.text_arg(&args[2])?;
                Ok((format!("replace({}, {}, {})", t, old, new), Cat::Text))
            }
            "Text.Clean" => {
                arity(1, 1)?;
                Ok((clean_sql(&self.text_arg(&args[0])?), Cat::Text))
            }
            "Text.Reverse" => {
                arity(1, 1)?;
                Ok((format!("reverse({})", self.text_arg(&args[0])?), Cat::Text))
            }
            "Text.Repeat" => {
                arity(2, 2)?;
                let t = self.text_arg(&args[0])?;
                let k = self.num_arg(&args[1])?;
                Ok((format!("repeat({}, CAST({} AS BIGINT))", t, k), Cat::Text))
            }
            // ---- Rodada 2: número ----
            "Number.Power" => {
                arity(2, 2)?;
                Ok((
                    format!("pow(CAST({} AS DOUBLE), CAST({} AS DOUBLE))", self.num_arg(&args[0])?, self.num_arg(&args[1])?),
                    Cat::Float,
                ))
            }
            "Number.Sqrt" => {
                arity(1, 1)?;
                Ok((format!("sqrt(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.Mod" => {
                arity(2, 2)?;
                let x = self.num_arg(&args[0])?;
                let y = self.num_arg(&args[1])?;
                Ok((
                    format!(
                        "(CAST({x} AS DOUBLE) - CAST({y} AS DOUBLE) * floor(CAST({x} AS DOUBLE) / CAST({y} AS DOUBLE)))",
                        x = x,
                        y = y
                    ),
                    Cat::Float,
                ))
            }
            "Number.IntegerDivide" => {
                arity(2, 2)?;
                let x = self.num_arg(&args[0])?;
                let y = self.num_arg(&args[1])?;
                Ok((
                    format!("CAST(floor(CAST({x} AS DOUBLE) / CAST({y} AS DOUBLE)) AS BIGINT)", x = x, y = y),
                    Cat::Int,
                ))
            }
            "Number.RoundUp" => {
                arity(1, 1)?;
                Ok((format!("ceil(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.RoundDown" => {
                arity(1, 1)?;
                Ok((format!("floor(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.Sign" => {
                arity(1, 1)?;
                Ok((format!("CAST(sign(CAST({} AS DOUBLE)) AS BIGINT)", self.num_arg(&args[0])?), Cat::Int))
            }
            "Number.Ln" => {
                arity(1, 1)?;
                Ok((format!("ln(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.Log10" => {
                arity(1, 1)?;
                Ok((format!("log10(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.Exp" => {
                arity(1, 1)?;
                Ok((format!("exp(CAST({} AS DOUBLE))", self.num_arg(&args[0])?), Cat::Float))
            }
            "Number.IsEven" | "Number.IsOdd" => {
                arity(1, 1)?;
                let x = self.num_arg(&args[0])?;
                let sym = if f == "Number.IsEven" { "= 0" } else { "<> 0" };
                Ok((format!("((CAST({} AS BIGINT) % 2) {})", x, sym), Cat::Bool))
            }
            // ---- Rodada 2: data ----
            "Date.QuarterOfYear" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("CAST(quarter({}) AS BIGINT)", d), Cat::Int))
            }
            "Date.WeekOfYear" => {
                arity(1, 2)?;
                let (d, _) = self.date_arg(&args[0])?;
                let k = first_day_index(args.get(1))?;
                Ok((
                    format!(
                        "CAST((date_part('doy', {d}) - 1 + ((date_part('dow', date_trunc('year', {d})) - {k} + 7) % 7)) // 7 + 1 AS BIGINT)",
                        d = d,
                        k = k
                    ),
                    Cat::Int,
                ))
            }
            "Date.DayOfYear" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("CAST(date_part('doy', {}) AS BIGINT)", d), Cat::Int))
            }
            "Date.DaysInMonth" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("CAST(day(last_day({})) AS BIGINT)", d), Cat::Int))
            }
            "Date.StartOfMonth" => {
                arity(1, 1)?;
                date_trunc_fn(self, &args[0], "month")
            }
            "Date.StartOfYear" => {
                arity(1, 1)?;
                date_trunc_fn(self, &args[0], "year")
            }
            "Date.StartOfQuarter" => {
                arity(1, 1)?;
                date_trunc_fn(self, &args[0], "quarter")
            }
            "Date.EndOfMonth" => {
                arity(1, 1)?;
                let (d, c) = self.date_arg(&args[0])?;
                Ok(if c == Cat::Timestamp {
                    (
                        format!(
                            "(CAST(last_day(CAST({d} AS DATE)) AS TIMESTAMP) + to_microseconds(86399999999))",
                            d = d
                        ),
                        Cat::Timestamp,
                    )
                } else {
                    (format!("last_day({})", d), Cat::Date)
                })
            }
            "Date.EndOfYear" => end_of_period(self, &args[0], "year"),
            "Date.EndOfQuarter" => end_of_period(self, &args[0], "quarter"),
            "Date.StartOfWeek" => {
                arity(1, 2)?;
                let (d, c) = self.date_arg(&args[0])?;
                let k = first_day_index(args.get(1))?;
                let cast = if c == Cat::Timestamp { "DATE" } else { "" };
                let base = if cast.is_empty() { d.clone() } else { format!("CAST({} AS DATE)", d) };
                let expr = format!("({} - to_days(CAST((date_part('dow', {}) - {} + 7) % 7 AS INTEGER)))", base, d, k);
                Ok(if c == Cat::Timestamp {
                    (format!("CAST({} AS TIMESTAMP)", expr), Cat::Timestamp)
                } else {
                    (format!("CAST({} AS DATE)", expr), Cat::Date)
                })
            }
            "Date.EndOfWeek" => {
                arity(1, 2)?;
                let (d, c) = self.date_arg(&args[0])?;
                let k = first_day_index(args.get(1))?;
                let base = if c == Cat::Timestamp { format!("CAST({} AS DATE)", d) } else { d.clone() };
                let start = format!("({} - to_days(CAST((date_part('dow', {}) - {} + 7) % 7 AS INTEGER)))", base, d, k);
                let expr = format!("({} + to_days(6))", start);
                Ok(if c == Cat::Timestamp {
                    (format!("CAST({} AS TIMESTAMP)", expr), Cat::Timestamp)
                } else {
                    (format!("CAST({} AS DATE)", expr), Cat::Date)
                })
            }
            "Date.AddYears" | "Date.AddQuarters" | "Date.AddWeeks" => {
                arity(2, 2)?;
                let (d, c) = self.date_arg(&args[0])?;
                let k = self.num_arg(&args[1])?;
                let expr = match f {
                    "Date.AddYears" => format!("({} + to_months(CAST({} AS INTEGER) * 12))", d, k),
                    "Date.AddQuarters" => format!("({} + to_months(CAST({} AS INTEGER) * 3))", d, k),
                    _ => format!("({} + to_days(CAST({} AS INTEGER) * 7))", d, k),
                };
                Ok(if c == Cat::Timestamp {
                    (expr, Cat::Timestamp)
                } else {
                    (format!("CAST({} AS DATE)", expr), Cat::Date)
                })
            }
            "Date.IsInCurrentYear" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("(year({}) = year(current_date))", d), Cat::Bool))
            }
            "DateTime.Date" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("CAST({} AS DATE)", d), Cat::Date))
            }
            "DateTime.Time" => {
                arity(1, 1)?;
                let (d, _) = self.date_arg(&args[0])?;
                Ok((format!("CAST({} AS TIME)", d), Cat::Other))
            }
            "DateTime.LocalNow" => {
                arity(0, 0)?;
                Ok(("current_timestamp".to_string(), Cat::Timestamp))
            }
            "Date.ToText" => {
                arity(2, 3)?;
                let (d, _) = self.date_arg(&args[0])?;
                let fmt = args
                    .get(1)
                    .and_then(|e| e.unparen().as_text())
                    .ok_or("Date.ToText exige um formato de texto literal")?;
                let cul = self.culture_arg(args.get(2))?;
                let (months, _) = name_tables(cul.as_deref())?;
                Ok((date_to_text_sql(&d, fmt, months)?, Cat::Text))
            }
            "Duration.Days" => {
                arity(1, 1)?;
                let inner = args[0].unparen();
                let ExprKind::Binary { op: BinOp::Sub, left, right } = &inner.kind else {
                    return Err("Duration.Days espera uma subtração de datas (a - b)".into());
                };
                let (l, _) = self.date_arg(left)?;
                let (r, _) = self.date_arg(right)?;
                Ok((
                    format!("CAST(date_diff('day', CAST({} AS DATE), CAST({} AS DATE)) AS BIGINT)", r, l),
                    Cat::Int,
                ))
            }
            other => Err(unsupported(other)),
        }
    }
}

/// `Date.StartOfMonth/Year/Quarter`: `date_trunc` keeps the input's category.
fn date_trunc_fn(tr: &Tr<'_>, arg: &Expr, part: &str) -> R {
    let (d, c) = tr.date_arg(arg)?;
    let expr = format!("date_trunc('{}', {})", part, d);
    Ok(if c == Cat::Timestamp { (expr, Cat::Timestamp) } else { (format!("CAST({} AS DATE)", expr), Cat::Date) })
}

/// `Date.EndOfYear/Quarter`: last day of the period, time preserved for datetime.
fn end_of_period(tr: &Tr<'_>, arg: &Expr, part: &str) -> R {
    let (d, c) = tr.date_arg(arg)?;
    let next = format!("(date_trunc('{}', {}) + to_months(CASE '{}' WHEN 'year' THEN 12 ELSE 3 END))", part, d, part);
    let last_date = format!("CAST({} - to_days(1) AS DATE)", next);
    Ok(if c == Cat::Timestamp {
        (format!("(CAST({} AS TIMESTAMP) + to_microseconds(86399999999))", last_date), Cat::Timestamp)
    } else {
        (last_date, Cat::Date)
    })
}

/// `Date.ToText`: supports only `yyyy yy MM M dd d MMM MMMM` tokens; any other
/// character in `fmt` is copied verbatim.
fn date_to_text_sql(d: &str, fmt: &str, months: &[&str; 12]) -> Result<String, String> {
    const TOKENS: [&str; 8] = ["yyyy", "MMMM", "MMM", "yy", "MM", "dd", "M", "d"];
    let mut parts: Vec<String> = Vec::new();
    let mut rest = fmt;
    while !rest.is_empty() {
        if let Some(tok) = TOKENS.iter().find(|t| rest.starts_with(**t)) {
            let piece = match *tok {
                "yyyy" => format!("lpad(CAST(year({}) AS VARCHAR), 4, '0')", d),
                "yy" => format!("lpad(CAST(CAST(year({}) AS BIGINT) % 100 AS VARCHAR), 2, '0')", d),
                "MMMM" => format!("list_extract({}, month({}))", list_lit(months), d),
                "MMM" => format!("substr(list_extract({}, month({})), 1, 3)", list_lit(months), d),
                "MM" => format!("lpad(CAST(month({}) AS VARCHAR), 2, '0')", d),
                "M" => format!("CAST(month({}) AS VARCHAR)", d),
                "dd" => format!("lpad(CAST(day({}) AS VARCHAR), 2, '0')", d),
                "d" => format!("CAST(day({}) AS VARCHAR)", d),
                _ => unreachable!(),
            };
            parts.push(piece);
            rest = &rest[tok.len()..];
        } else {
            let mut cut = rest.len();
            for i in 1..rest.len() {
                if TOKENS.iter().any(|t| rest[i..].starts_with(t)) {
                    cut = i;
                    break;
                }
            }
            parts.push(lit_str(&rest[..cut]));
            rest = &rest[cut..];
        }
    }
    if parts.is_empty() {
        return Ok(lit_str(""));
    }
    Ok(format!("({})", parts.join(" || ")))
}

/// Translates a custom-column formula (with or without a leading `each`).
pub fn translate(expression: &str, chain: &Chain) -> Result<(String, Cat), String> {
    translate_inner(expression, chain, None)
}

/// Translates a `transformColumns` formula where `_` is bound to `cell`
/// (SQL expression, category) of the current column's value.
pub fn translate_cell(expression: &str, chain: &Chain, cell: (String, Cat)) -> Result<(String, Cat), String> {
    translate_inner(expression, chain, Some(cell))
}

fn translate_inner(expression: &str, chain: &Chain, underscore: Option<(String, Cat)>) -> Result<(String, Cat), String> {
    let e = parse_expression(expression.trim()).map_err(|e| format!("fórmula inválida: {}", e))?;
    let body = match &e.kind {
        ExprKind::Each(b) => b.as_ref(),
        _ => &e,
    };
    Tr { chain, underscore }.tr(body)
}
