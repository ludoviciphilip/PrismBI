//! Tokenizer + recursive-descent parser for the Power Query M subset used by
//! real reports: `let … in`, `#"quoted identifiers"`, records `[..]`, lists
//! `{..}`, `each`/`_`, implicit field access `[Col]`, item access
//! `Source{[Item=..,Kind=..]}[Data]`, calls, type literals (`type date`,
//! `Int64.Type`, …), `if/then/else`, `and/or/not`, function literals, `try`.
//!
//! Every node carries the byte span of its source text, so unsupported pieces
//! can be preserved verbatim (opaque steps) and custom-column expressions can
//! be stored exactly as written.

use std::fmt;

pub type Span = (usize, usize);

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub expr: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Concat,
    Coalesce,
    Meta,
    Is,
    As,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// `type date`, `type nullable text`, …
    Primitive { name: String, nullable: bool },
    /// `type table [..]`, `type {..}`, `type [..]`, `type function ..`: not interpreted.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Null,
    Bool(bool),
    Num(f64),
    Text(String),
    /// Regular, dotted (`Table.AddColumn`) or quoted (`#"Changed Type"`) identifier,
    /// and `#`-keywords (`#date`, `#table`, …).
    Ident(String),
    Let {
        bindings: Vec<Binding>,
        body: Box<Expr>,
    },
    Each(Box<Expr>),
    Func {
        params: Vec<String>,
        body: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    Record(Vec<(String, Expr)>),
    List(Vec<Expr>),
    /// `[Col]` (target None = implicit `_`) or `x[Col]`.
    Field {
        target: Option<Box<Expr>>,
        name: String,
        optional: bool,
    },
    /// `x{key}`
    Item {
        target: Box<Expr>,
        key: Box<Expr>,
        optional: bool,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Type(TypeExpr),
    Paren(Box<Expr>),
    Try {
        expr: Box<Expr>,
        otherwise: Option<Box<Expr>>,
    },
    Error(Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    NotImplemented,
    /// A `let` binding whose expression could not be parsed (error recovery);
    /// the original text is preserved.
    Opaque(String),
}

impl Expr {
    fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Source text of this node.
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        src.get(self.span.0..self.span.1).unwrap_or("").trim()
    }

    pub fn as_ident(&self) -> Option<&str> {
        match &self.kind {
            ExprKind::Ident(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match &self.kind {
            ExprKind::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Number literal, accepting a unary minus/plus in front.
    pub fn as_number(&self) -> Option<f64> {
        match &self.kind {
            ExprKind::Num(n) => Some(*n),
            ExprKind::Unary { op: UnOp::Neg, expr } => expr.as_number().map(|n| -n),
            ExprKind::Unary { op: UnOp::Plus, expr } => expr.as_number(),
            ExprKind::Paren(e) => e.as_number(),
            _ => None,
        }
    }

    /// `Fn.Name(args)` → (`"Fn.Name"`, args).
    pub fn as_call(&self) -> Option<(&str, &[Expr])> {
        match &self.kind {
            ExprKind::Call { func, args } => func.as_ident().map(|n| (n, args.as_slice())),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Expr]> {
        match &self.kind {
            ExprKind::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.kind, ExprKind::Null)
    }

    /// Strip any number of redundant parentheses.
    pub fn unparen(&self) -> &Expr {
        match &self.kind {
            ExprKind::Paren(e) => e.unparen(),
            _ => self,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub pos: usize,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (linha {}, coluna {})", self.message, self.line, self.col)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    /// `#date`, `#table`, `#duration`, …
    Hash(String),
    QuotedIdent(String),
    Text(String),
    Num(f64),
    Punct(&'static str),
    Eof,
}

const KEYWORDS: &[&str] = &[
    "and", "as", "each", "else", "error", "false", "if", "in", "is", "let", "meta", "not", "null",
    "or", "otherwise", "section", "shared", "then", "true", "try", "type",
];

pub fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

const PUNCTS: &[&str] = &[
    "...", "=>", "<=", ">=", "<>", "..", "??", "=", "<", ">", "+", "-", "*", "/", "&", "(", ")",
    "[", "]", "{", "}", ",", ";", "?", "@", "!",
];

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

fn is_ident_part(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    peeked: Option<(Tok, usize, usize)>,
    last_end: usize,
}

type PResult<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            pos: 0,
            peeked: None,
            last_end: 0,
        }
    }

    fn err_at<T>(&self, pos: usize, msg: impl Into<String>) -> PResult<T> {
        let before = &self.src[..pos.min(self.src.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0) + 1;
        Err(ParseError {
            message: msg.into(),
            pos,
            line,
            col,
        })
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.src.get(i..).and_then(|s| s.chars().next())
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        loop {
            let Some(c) = self.char_at(i) else { return i };
            if c.is_whitespace() || c == '\u{feff}' {
                i += c.len_utf8();
            } else if self.src[i..].starts_with("//") {
                match self.src[i..].find('\n') {
                    Some(n) => i += n + 1,
                    None => return self.src.len(),
                }
            } else if self.src[i..].starts_with("/*") {
                match self.src[i + 2..].find("*/") {
                    Some(n) => i += 2 + n + 2,
                    None => return self.src.len(),
                }
            } else {
                return i;
            }
        }
    }

    fn scan_ident(&self, start: usize) -> usize {
        let mut i = start;
        loop {
            match self.char_at(i) {
                Some(c) if is_ident_part(c) => i += c.len_utf8(),
                Some('.') => match self.char_at(i + 1) {
                    Some(n) if is_ident_start(n) => i += 1,
                    _ => return i,
                },
                _ => return i,
            }
        }
    }

    /// Reads a `"..."` literal starting at `start` (the opening quote).
    fn scan_text(&self, start: usize) -> PResult<(String, usize)> {
        let mut out = String::new();
        let mut i = start + 1;
        loop {
            let Some(c) = self.char_at(i) else {
                return self.err_at(start, "texto sem aspas de fechamento");
            };
            if c == '"' {
                if self.char_at(i + 1) == Some('"') {
                    out.push('"');
                    i += 2;
                    continue;
                }
                return Ok((out, i + 1));
            }
            if c == '#' && self.char_at(i + 1) == Some('(') {
                if let Some(close) = self.src[i + 2..].find(')') {
                    let body = &self.src[i + 2..i + 2 + close];
                    if let Some(decoded) = decode_escape(body) {
                        out.push_str(&decoded);
                        i += 2 + close + 1;
                        continue;
                    }
                }
            }
            out.push(c);
            i += c.len_utf8();
        }
    }

    fn lex_at(&self, at: usize) -> PResult<(Tok, usize, usize)> {
        let start = self.skip_ws(at);
        let Some(c) = self.char_at(start) else {
            return Ok((Tok::Eof, start, start));
        };
        if c == '#' {
            if self.char_at(start + 1) == Some('"') {
                let (s, end) = self.scan_text(start + 1)?;
                return Ok((Tok::QuotedIdent(s), start, end));
            }
            if self.char_at(start + 1).is_some_and(is_ident_start) {
                let end = self.scan_ident(start + 1);
                return Ok((Tok::Hash(self.src[start..end].to_string()), start, end));
            }
            return self.err_at(start, "'#' inesperado");
        }
        if c == '"' {
            let (s, end) = self.scan_text(start)?;
            return Ok((Tok::Text(s), start, end));
        }
        if c.is_ascii_digit()
            || (c == '.' && self.char_at(start + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            return self.lex_number(start);
        }
        if is_ident_start(c) {
            let end = self.scan_ident(start);
            return Ok((Tok::Ident(self.src[start..end].to_string()), start, end));
        }
        for p in PUNCTS {
            if self.src[start..].starts_with(p) {
                return Ok((Tok::Punct(p), start, start + p.len()));
            }
        }
        self.err_at(start, format!("caractere inesperado '{}'", c))
    }

    fn lex_number(&self, start: usize) -> PResult<(Tok, usize, usize)> {
        let s = &self.src[start..];
        if s.starts_with("0x") || s.starts_with("0X") {
            let digits: String = s[2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            let v = i64::from_str_radix(&digits, 16)
                .map_err(|_| ())
                .or_else(|_| self.err_at(start, "número hexadecimal inválido"))?;
            return Ok((Tok::Num(v as f64), start, start + 2 + digits.len()));
        }
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
            let mut j = i + 1;
            if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
                j += 1;
            }
            if j < bytes.len() && bytes[j].is_ascii_digit() {
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        let v: f64 = s[..i]
            .parse()
            .map_err(|_| ())
            .or_else(|_| self.err_at(start, "número inválido"))?;
        Ok((Tok::Num(v), start, start + i))
    }

    fn peek(&mut self) -> PResult<&Tok> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lex_at(self.pos)?);
        }
        Ok(&self.peeked.as_ref().unwrap().0)
    }

    fn peek_start(&mut self) -> PResult<usize> {
        self.peek()?;
        Ok(self.peeked.as_ref().unwrap().1)
    }

    fn bump(&mut self) -> PResult<(Tok, usize, usize)> {
        let t = match self.peeked.take() {
            Some(t) => t,
            None => self.lex_at(self.pos)?,
        };
        self.pos = t.2;
        self.last_end = t.2;
        Ok(t)
    }

    fn reset_to(&mut self, pos: usize) {
        self.pos = pos;
        self.peeked = None;
    }

    fn is_punct(&mut self, p: &str) -> PResult<bool> {
        Ok(matches!(self.peek()?, Tok::Punct(x) if *x == p))
    }

    fn is_kw(&mut self, kw: &str) -> PResult<bool> {
        Ok(matches!(self.peek()?, Tok::Ident(x) if x == kw))
    }

    fn expect_punct(&mut self, p: &str) -> PResult<usize> {
        if self.is_punct(p)? {
            Ok(self.bump()?.1)
        } else {
            let at = self.peek_start()?;
            let found = format!("{:?}", self.peek()?);
            self.err_at(at, format!("esperado '{}', encontrado {}", p, found))
        }
    }

    fn expect_kw(&mut self, kw: &str) -> PResult<usize> {
        if self.is_kw(kw)? {
            Ok(self.bump()?.1)
        } else {
            let at = self.peek_start()?;
            self.err_at(at, format!("esperada a palavra-chave '{}'", kw))
        }
    }

    // ---------------------------------------------------------------- grammar

    fn parse_expr(&mut self) -> PResult<Expr> {
        let start = self.peek_start()?;
        if self.is_kw("let")? {
            return self.parse_let();
        }
        if self.is_kw("each")? {
            self.bump()?;
            let body = self.parse_expr()?;
            return Ok(Expr::new(ExprKind::Each(Box::new(body)), (start, self.last_end)));
        }
        if self.is_kw("if")? {
            self.bump()?;
            let cond = self.parse_expr()?;
            self.expect_kw("then")?;
            let then = self.parse_expr()?;
            self.expect_kw("else")?;
            let els = self.parse_expr()?;
            return Ok(Expr::new(
                ExprKind::If {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    els: Box::new(els),
                },
                (start, self.last_end),
            ));
        }
        if self.is_kw("try")? {
            self.bump()?;
            let expr = self.parse_expr()?;
            let otherwise = if self.is_kw("otherwise")? {
                self.bump()?;
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            return Ok(Expr::new(
                ExprKind::Try {
                    expr: Box::new(expr),
                    otherwise,
                },
                (start, self.last_end),
            ));
        }
        if self.is_kw("error")? {
            self.bump()?;
            let e = self.parse_expr()?;
            return Ok(Expr::new(ExprKind::Error(Box::new(e)), (start, self.last_end)));
        }
        if self.is_punct("(")? {
            if let Some(f) = self.try_parse_function()? {
                return Ok(f);
            }
        }
        self.parse_coalesce()
    }

    /// `(a, b as text) as number => body`; returns None (and rewinds) if the
    /// parenthesis is not a function literal.
    fn try_parse_function(&mut self) -> PResult<Option<Expr>> {
        let save = self.pos;
        let save_peek = self.peeked.clone();
        let save_end = self.last_end;
        let start = self.peek_start()?;
        let attempt: PResult<Option<Expr>> = (|| {
            self.expect_punct("(")?;
            let mut params = Vec::new();
            if !self.is_punct(")")? {
                loop {
                    if self.is_kw("optional")? {
                        self.bump()?;
                    }
                    let name = match self.bump()?.0 {
                        Tok::Ident(n) if !is_keyword(&n) => n,
                        Tok::QuotedIdent(n) => n,
                        _ => return Ok(None),
                    };
                    if self.is_kw("as")? {
                        self.bump()?;
                        self.skip_type_ref()?;
                    }
                    params.push(name);
                    if self.is_punct(",")? {
                        self.bump()?;
                        continue;
                    }
                    break;
                }
            }
            if !self.is_punct(")")? {
                return Ok(None);
            }
            self.bump()?;
            if self.is_kw("as")? {
                self.bump()?;
                self.skip_type_ref()?;
            }
            if !self.is_punct("=>")? {
                return Ok(None);
            }
            self.bump()?;
            let body = self.parse_expr()?;
            Ok(Some(Expr::new(
                ExprKind::Func {
                    params,
                    body: Box::new(body),
                },
                (start, self.last_end),
            )))
        })();
        match attempt {
            Ok(Some(e)) => Ok(Some(e)),
            Ok(None) | Err(_) => {
                self.pos = save;
                self.peeked = save_peek;
                self.last_end = save_end;
                Ok(None)
            }
        }
    }

    /// Skips `nullable text`, `number`, `Int64.Type`, `table`, …
    fn skip_type_ref(&mut self) -> PResult<()> {
        if self.is_kw("nullable")? {
            self.bump()?;
        }
        match self.bump()?.0 {
            Tok::Ident(_) => Ok(()),
            _ => {
                let at = self.last_end;
                self.err_at(at, "tipo esperado")
            }
        }
    }

    fn parse_let(&mut self) -> PResult<Expr> {
        let start = self.expect_kw("let")?;
        let mut bindings = Vec::new();
        loop {
            let name = match self.bump()? {
                (Tok::Ident(n), _, _) if !is_keyword(&n) => n,
                (Tok::QuotedIdent(n), _, _) => n,
                (_, s, _) => return self.err_at(s, "nome de etapa esperado no 'let'"),
            };
            self.expect_punct("=")?;
            let expr_start = self.skip_ws(self.pos);
            let expr = match self.parse_expr() {
                Ok(e) if self.is_punct(",")? || self.is_kw("in")? => e,
                _ => {
                    // Error recovery: keep the raw text up to the next top-level ',' / 'in'.
                    let end = scan_binding_end(self.src, expr_start);
                    self.reset_to(end);
                    self.last_end = end;
                    let raw = self.src[expr_start..end].trim().to_string();
                    let trimmed_end = expr_start + self.src[expr_start..end].trim_end().len();
                    Expr::new(ExprKind::Opaque(raw), (expr_start, trimmed_end))
                }
            };
            bindings.push(Binding { name, expr });
            if self.is_punct(",")? {
                self.bump()?;
                continue;
            }
            break;
        }
        self.expect_kw("in")?;
        let body = self.parse_expr()?;
        Ok(Expr::new(
            ExprKind::Let {
                bindings,
                body: Box::new(body),
            },
            (start, self.last_end),
        ))
    }

    fn parse_binary_level(
        &mut self,
        ops: &[(&str, BinOp, bool)],
        next: fn(&mut Self) -> PResult<Expr>,
    ) -> PResult<Expr> {
        let mut left = next(self)?;
        'outer: loop {
            for (tok, op, is_kw) in ops {
                let hit = if *is_kw { self.is_kw(tok)? } else { self.is_punct(tok)? };
                if hit {
                    self.bump()?;
                    let right = next(self)?;
                    let span = (left.span.0, right.span.1);
                    left = Expr::new(
                        ExprKind::Binary {
                            op: *op,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        span,
                    );
                    continue 'outer;
                }
            }
            return Ok(left);
        }
    }

    fn parse_coalesce(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("??", BinOp::Coalesce, false)], Self::parse_or)
    }
    fn parse_or(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("or", BinOp::Or, true)], Self::parse_and)
    }
    fn parse_and(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("and", BinOp::And, true)], Self::parse_is)
    }
    fn parse_is(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("is", BinOp::Is, true), ("as", BinOp::As, true)], Self::parse_eq)
    }
    fn parse_eq(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("=", BinOp::Eq, false), ("<>", BinOp::Ne, false)], Self::parse_rel)
    }
    fn parse_rel(&mut self) -> PResult<Expr> {
        self.parse_binary_level(
            &[
                ("<=", BinOp::Le, false),
                (">=", BinOp::Ge, false),
                ("<", BinOp::Lt, false),
                (">", BinOp::Gt, false),
            ],
            Self::parse_add,
        )
    }
    fn parse_add(&mut self) -> PResult<Expr> {
        self.parse_binary_level(
            &[("+", BinOp::Add, false), ("-", BinOp::Sub, false), ("&", BinOp::Concat, false)],
            Self::parse_mul,
        )
    }
    fn parse_mul(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("*", BinOp::Mul, false), ("/", BinOp::Div, false)], Self::parse_meta)
    }
    fn parse_meta(&mut self) -> PResult<Expr> {
        self.parse_binary_level(&[("meta", BinOp::Meta, true)], Self::parse_unary)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let start = self.peek_start()?;
        let op = if self.is_punct("-")? {
            Some(UnOp::Neg)
        } else if self.is_punct("+")? {
            Some(UnOp::Plus)
        } else if self.is_kw("not")? {
            Some(UnOp::Not)
        } else {
            None
        };
        if let Some(op) = op {
            self.bump()?;
            let e = self.parse_unary()?;
            return Ok(Expr::new(
                ExprKind::Unary {
                    op,
                    expr: Box::new(e),
                },
                (start, self.last_end),
            ));
        }
        if self.is_kw("type")? {
            self.bump()?;
            let t = self.parse_type_body()?;
            return Ok(Expr::new(ExprKind::Type(t), (start, self.last_end)));
        }
        self.parse_postfix()
    }

    fn parse_type_body(&mut self) -> PResult<TypeExpr> {
        let mut nullable = false;
        if self.is_kw("nullable")? {
            self.bump()?;
            nullable = true;
        }
        if self.is_punct("[")? || self.is_punct("{")? {
            self.skip_balanced()?;
            return Ok(TypeExpr::Other);
        }
        match self.peek()?.clone() {
            Tok::Ident(name) => {
                self.bump()?;
                if (name == "table" && self.is_punct("[")?) || (name == "function" && self.is_punct("(")?) {
                    self.skip_balanced()?;
                    if name == "function" && self.is_kw("as")? {
                        self.bump()?;
                        self.skip_type_ref()?;
                    }
                    return Ok(TypeExpr::Other);
                }
                Ok(TypeExpr::Primitive { name, nullable })
            }
            _ => {
                let at = self.peek_start()?;
                self.err_at(at, "tipo esperado após 'type'")
            }
        }
    }

    /// Skips a balanced `[..]`, `{..}` or `(..)` group starting at the next token.
    fn skip_balanced(&mut self) -> PResult<()> {
        let start = self.peek_start()?;
        let end = scan_balanced(self.src, start)
            .ok_or(())
            .or_else(|_| self.err_at(start, "grupo sem fechamento"))?;
        self.reset_to(end);
        self.last_end = end;
        Ok(())
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            if self.is_punct("(")? {
                self.bump()?;
                let mut args = Vec::new();
                if !self.is_punct(")")? {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.is_punct(",")? {
                            self.bump()?;
                            continue;
                        }
                        break;
                    }
                }
                self.expect_punct(")")?;
                let span = (e.span.0, self.last_end);
                e = Expr::new(
                    ExprKind::Call {
                        func: Box::new(e),
                        args,
                    },
                    span,
                );
            } else if self.is_punct("{")? {
                self.bump()?;
                let key = self.parse_expr()?;
                self.expect_punct("}")?;
                let optional = if self.is_punct("?")? {
                    self.bump()?;
                    true
                } else {
                    false
                };
                let span = (e.span.0, self.last_end);
                e = Expr::new(
                    ExprKind::Item {
                        target: Box::new(e),
                        key: Box::new(key),
                        optional,
                    },
                    span,
                );
            } else if self.is_punct("[")? {
                let open = self.bump()?.1;
                let (name, after) = self
                    .scan_generalized_ident(open + 1)
                    .ok_or(())
                    .or_else(|_| self.err_at(open, "acesso a campo inválido"))?;
                self.reset_to(after);
                self.expect_punct("]")?;
                let optional = if self.is_punct("?")? {
                    self.bump()?;
                    true
                } else {
                    false
                };
                let span = (e.span.0, self.last_end);
                e = Expr::new(
                    ExprKind::Field {
                        target: Some(Box::new(e)),
                        name,
                        optional,
                    },
                    span,
                );
            } else {
                return Ok(e);
            }
        }
    }

    /// Generalized identifier inside `[...]`: either `#"..."` or raw text up
    /// to `]`, `=` or `,`. Returns (name, position after the name).
    fn scan_generalized_ident(&self, from: usize) -> Option<(String, usize)> {
        let i = self.skip_ws(from);
        if self.src[i..].starts_with("#\"") {
            let (s, end) = self.scan_text(i + 1).ok()?;
            return Some((s, end));
        }
        let rest = &self.src[i..];
        let stop = rest.find([']', '=', ','])?;
        let raw = &rest[..stop];
        let name = raw.trim();
        if name.is_empty() || name.contains(['[', '{', '"', '(', ')', '\n']) {
            return None;
        }
        Some((name.to_string(), i + raw.trim_end().len()))
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let (tok, start, end) = {
            self.peek()?;
            self.peeked.clone().unwrap()
        };
        match tok {
            Tok::Num(n) => {
                self.bump()?;
                Ok(Expr::new(ExprKind::Num(n), (start, end)))
            }
            Tok::Text(s) => {
                self.bump()?;
                Ok(Expr::new(ExprKind::Text(s), (start, end)))
            }
            Tok::QuotedIdent(s) => {
                self.bump()?;
                Ok(Expr::new(ExprKind::Ident(s), (start, end)))
            }
            Tok::Hash(h) => {
                self.bump()?;
                let kind = match h.as_str() {
                    "#infinity" => ExprKind::Num(f64::INFINITY),
                    "#nan" => ExprKind::Num(f64::NAN),
                    _ => ExprKind::Ident(h),
                };
                Ok(Expr::new(kind, (start, end)))
            }
            Tok::Ident(w) => {
                self.bump()?;
                let kind = match w.as_str() {
                    "null" => ExprKind::Null,
                    "true" => ExprKind::Bool(true),
                    "false" => ExprKind::Bool(false),
                    kw if is_keyword(kw) => {
                        return self.err_at(start, format!("palavra-chave '{}' inesperada", kw))
                    }
                    _ => ExprKind::Ident(w),
                };
                Ok(Expr::new(kind, (start, end)))
            }
            Tok::Punct("(") => {
                self.bump()?;
                let inner = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(Expr::new(ExprKind::Paren(Box::new(inner)), (start, self.last_end)))
            }
            Tok::Punct("{") => {
                self.bump()?;
                let mut items = Vec::new();
                if !self.is_punct("}")? {
                    loop {
                        let item = self.parse_expr()?;
                        let item = if self.is_punct("..")? {
                            self.bump()?;
                            let hi = self.parse_expr()?;
                            let span = (item.span.0, hi.span.1);
                            Expr::new(ExprKind::Range(Box::new(item), Box::new(hi)), span)
                        } else {
                            item
                        };
                        items.push(item);
                        if self.is_punct(",")? {
                            self.bump()?;
                            continue;
                        }
                        break;
                    }
                }
                self.expect_punct("}")?;
                Ok(Expr::new(ExprKind::List(items), (start, self.last_end)))
            }
            Tok::Punct("[") => {
                self.bump()?;
                self.parse_record_or_field(start)
            }
            Tok::Punct("@") => {
                self.bump()?;
                self.parse_primary()
            }
            Tok::Punct("...") => {
                self.bump()?;
                Ok(Expr::new(ExprKind::NotImplemented, (start, end)))
            }
            Tok::Eof => self.err_at(start, "fim inesperado da expressão"),
            other => self.err_at(start, format!("token inesperado {:?}", other)),
        }
    }

    /// After `[`: `[]`, `[A = 1, B = 2]` (record) or `[Col]` (implicit field access).
    fn parse_record_or_field(&mut self, open: usize) -> PResult<Expr> {
        if self.is_punct("]")? {
            self.bump()?;
            return Ok(Expr::new(ExprKind::Record(vec![]), (open, self.last_end)));
        }
        let Some((first, after)) = self.scan_generalized_ident(open + 1) else {
            return self.err_at(open, "registro ou acesso a campo inválido");
        };
        self.reset_to(after);
        if self.is_punct("]")? {
            self.bump()?;
            let optional = if self.is_punct("?")? {
                self.bump()?;
                true
            } else {
                false
            };
            return Ok(Expr::new(
                ExprKind::Field {
                    target: None,
                    name: first,
                    optional,
                },
                (open, self.last_end),
            ));
        }
        let mut fields = Vec::new();
        let mut name = first;
        loop {
            self.expect_punct("=")?;
            let value = self.parse_expr()?;
            fields.push((name, value));
            if self.is_punct(",")? {
                let comma = self.bump()?.2;
                let Some((n, after)) = self.scan_generalized_ident(comma) else {
                    return self.err_at(comma, "nome de campo esperado");
                };
                self.reset_to(after);
                name = n;
                continue;
            }
            break;
        }
        self.expect_punct("]")?;
        Ok(Expr::new(ExprKind::Record(fields), (open, self.last_end)))
    }
}

fn decode_escape(body: &str) -> Option<String> {
    let mut out = String::new();
    for part in body.split(',') {
        match part.trim() {
            "cr" => out.push('\r'),
            "lf" => out.push('\n'),
            "tab" => out.push('\t'),
            "#" => out.push('#'),
            hex if (hex.len() == 4 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                out.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?)
            }
            _ => return None,
        }
    }
    if body.trim() == "#" {
        // `#(#)` is the escape for a literal "#(" prefix: emits "#" and the
        // caller continues with "(" as plain text.
        return Some("#".to_string());
    }
    Some(out)
}

/// End (exclusive) of a balanced group starting at `start` (`(`, `[` or `{`),
/// skipping strings, quoted identifiers and comments.
fn scan_balanced(src: &str, start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = start;
    let b = src.as_bytes();
    while i < b.len() {
        match b[i] {
            b'"' => i = skip_string(src, i),
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = src[i + 2..].find("*/").map(|n| i + 2 + n + 2).unwrap_or(b.len());
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn skip_string(src: &str, quote: usize) -> usize {
    let b = src.as_bytes();
    let mut i = quote + 1;
    while i < b.len() {
        if b[i] == b'"' {
            if b.get(i + 1) == Some(&b'"') {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

/// Error recovery for `let` bindings: end of the expression text starting at
/// `start`: the next top-level `,` / `in`, or — when brackets are unbalanced
/// (the usual reason the binding failed to parse) — a `,` that ends a line
/// followed by a line that starts a new binding (`Name =` / `#"Name" =`), or
/// a line that starts with the `in` keyword.
fn scan_binding_end(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut depth = 0i32;
    let mut i = start;
    let mut last_sig: Option<(usize, u8)> = None; // last non-whitespace byte outside strings/comments
    while i < b.len() {
        match b[i] {
            b'"' => {
                i = skip_string(src, i);
                last_sig = Some((i - 1, b'"'));
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = src[i + 2..].find("*/").map(|n| i + 2 + n + 2).unwrap_or(b.len());
                continue;
            }
            b'\n' => {
                let line = src[i + 1..].trim_start_matches([' ', '\t', '\r']);
                if starts_with_keyword(line, "in") {
                    return match last_sig {
                        Some((p, b',')) => p,
                        _ => i,
                    };
                }
                if let Some((p, b',')) = last_sig {
                    if starts_binding(line) {
                        return p;
                    }
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth <= 0 => return i,
            b'i' if depth <= 0
                && src[i..].starts_with("in")
                && (i == 0 || !src[..i].chars().next_back().is_some_and(is_ident_part))
                && !src[i + 2..].chars().next().is_some_and(|c| is_ident_part(c) || c == '.') =>
            {
                return i
            }
            _ => {}
        }
        if !b[i].is_ascii_whitespace() {
            last_sig = Some((i, b[i]));
        }
        i += 1;
    }
    b.len()
}

fn starts_with_keyword(line: &str, kw: &str) -> bool {
    line.starts_with(kw) && !line[kw.len()..].chars().next().is_some_and(|c| is_ident_part(c) || c == '.')
}

/// `Name =` or `#"Name" =` (not `==`/`=>`) at the start of `line`.
fn starts_binding(line: &str) -> bool {
    let rest = if let Some(q) = line.strip_prefix("#\"") {
        let mut i = 0;
        let bytes = q.as_bytes();
        loop {
            match bytes.get(i) {
                None => return false,
                Some(b'"') if bytes.get(i + 1) == Some(&b'"') => i += 2,
                Some(b'"') => break &q[i + 1..],
                Some(_) => i += 1,
            }
        }
    } else {
        let end = line
            .char_indices()
            .find(|(_, c)| !(is_ident_part(*c) || *c == '.'))
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        if end == 0 || !line.chars().next().is_some_and(is_ident_start) || is_keyword(&line[..end]) {
            return false;
        }
        &line[end..]
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    rest.starts_with('=') && !rest.starts_with("==") && !rest.starts_with("=>")
}

/// Parses a complete M document (usually `let … in …`).
pub fn parse_document(src: &str) -> Result<Expr, ParseError> {
    let mut p = Parser::new(src);
    let e = p.parse_expr()?;
    match p.peek()? {
        Tok::Eof => Ok(e),
        _ => {
            let at = p.peek_start()?;
            p.err_at(at, "texto extra após o fim da expressão")
        }
    }
}

/// Parses a standalone expression (e.g. a custom-column formula).
pub fn parse_expression(src: &str) -> Result<Expr, ParseError> {
    parse_document(src)
}

// ------------------------------------------------------------ text helpers

/// `#"..."` or bare identifier, as M requires.
pub fn quote_m_ident(name: &str) -> String {
    let mut chars = name.chars();
    let bare = match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    };
    if bare && !is_keyword(name) {
        name.to_string()
    } else {
        format!("#{}", quote_m_string(name))
    }
}

/// M text literal with `""` and `#(..)` escapes.
pub fn quote_m_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\"\""),
            '\r' => out.push_str("#(cr)"),
            '\n' => out.push_str("#(lf)"),
            '\t' => out.push_str("#(tab)"),
            '#' if chars.peek() == Some(&'(') => out.push_str("#(#)"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("#({:04X})", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// M number literal (integral values without a fraction).
pub fn format_m_number(n: f64) -> String {
    if n.is_nan() {
        "#nan".into()
    } else if n.is_infinite() {
        if n > 0.0 { "#infinity".into() } else { "-#infinity".into() }
    } else if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bindings(e: &Expr) -> &[Binding] {
        match &e.kind {
            ExprKind::Let { bindings, .. } => bindings,
            _ => panic!("let esperado"),
        }
    }

    #[test]
    fn parses_navigation_and_quoted_names() {
        let src = r#"let
    Source = Excel.Workbook(File.Contents("C:\Dados\a.xlsx"), null, true),
    Categorias_Sheet = Source{[Item="Categorias",Kind="Sheet"]}[Data],
    #"Promoted Headers" = Table.PromoteHeaders(Categorias_Sheet, [PromoteAllScalars=true])
in
    #"Promoted Headers""#;
        let doc = parse_document(src).unwrap();
        let b = bindings(&doc);
        assert_eq!(b.len(), 3);
        assert_eq!(b[2].name, "Promoted Headers");
        let (f, args) = b[0].expr.as_call().unwrap();
        assert_eq!(f, "Excel.Workbook");
        let (fc, fargs) = args[0].as_call().unwrap();
        assert_eq!(fc, "File.Contents");
        assert_eq!(fargs[0].as_text(), Some(r"C:\Dados\a.xlsx"));
        match &b[1].expr.kind {
            ExprKind::Field { target: Some(t), name, .. } => {
                assert_eq!(name, "Data");
                match &t.kind {
                    ExprKind::Item { key, .. } => match &key.kind {
                        ExprKind::Record(fields) => {
                            assert_eq!(fields[0].0, "Item");
                            assert_eq!(fields[0].1.as_text(), Some("Categorias"));
                        }
                        _ => panic!("record"),
                    },
                    _ => panic!("item"),
                }
            }
            _ => panic!("field"),
        }
    }

    #[test]
    fn parses_each_if_types_and_precedence() {
        let e = parse_expression(r#"each if [Mês] <= 6 and not ([A] = "x") then "S1" else "S2""#).unwrap();
        let ExprKind::Each(body) = &e.kind else { panic!() };
        let ExprKind::If { cond, .. } = &body.kind else { panic!() };
        let ExprKind::Binary { op: BinOp::And, left, right } = &cond.kind else { panic!() };
        assert!(matches!(left.kind, ExprKind::Binary { op: BinOp::Le, .. }));
        assert!(matches!(right.kind, ExprKind::Unary { op: UnOp::Not, .. }));

        let t = parse_expression("{{\"Data\", type date}, {\"Ordem\", Int64.Type}, {\"X\", type nullable text}}").unwrap();
        let items = t.as_list().unwrap();
        let pair = items[0].as_list().unwrap();
        assert_eq!(pair[1].kind, ExprKind::Type(TypeExpr::Primitive { name: "date".into(), nullable: false }));
        assert_eq!(items[1].as_list().unwrap()[1].as_ident(), Some("Int64.Type"));
        assert_eq!(
            items[2].as_list().unwrap()[1].kind,
            ExprKind::Type(TypeExpr::Primitive { name: "text".into(), nullable: true })
        );

        let arith = parse_expression("1 + 2 * 3 & \"a\"").unwrap();
        let ExprKind::Binary { op: BinOp::Concat, left, .. } = &arith.kind else { panic!() };
        assert!(matches!(left.kind, ExprKind::Binary { op: BinOp::Add, .. }));
    }

    #[test]
    fn spans_and_recovery() {
        let src = "let\n  A = 1,\n  B = Foo(A, [x = ), \n  C = A + 1\nin\n  C";
        let doc = parse_document(src).unwrap();
        let b = bindings(&doc);
        assert_eq!(b.len(), 3);
        assert!(matches!(&b[1].expr.kind, ExprKind::Opaque(t) if t == "Foo(A, [x = )"));
        assert_eq!(b[2].expr.text(src), "A + 1");
    }

    #[test]
    fn strings_escapes_and_comments() {
        let e = parse_expression("/* c */ \"a\"\"b#(lf)c\" // tail").unwrap();
        assert_eq!(e.as_text(), Some("a\"b\nc"));
        assert_eq!(quote_m_string("a\"b\nc"), "\"a\"\"b#(lf)c\"");
        assert_eq!(parse_expression(&quote_m_string("x#(y")).unwrap().as_text(), Some("x#(y"));
        assert_eq!(quote_m_ident("Changed Type"), "#\"Changed Type\"");
        assert_eq!(quote_m_ident("Source"), "Source");
        assert_eq!(quote_m_ident("each"), "#\"each\"");
    }

    #[test]
    fn function_literal_and_hash_calls() {
        let e = parse_expression("(x as number) as number => x * 2").unwrap();
        assert!(matches!(e.kind, ExprKind::Func { ref params, .. } if params == &["x".to_string()]));
        let d = parse_expression("Query1(#date(2026, 4, 1), 275, #duration(1, 0, 0, 0))").unwrap();
        let (f, args) = d.as_call().unwrap();
        assert_eq!(f, "Query1");
        assert_eq!(args[0].as_call().unwrap().0, "#date");
        assert_eq!(args[1].as_number(), Some(275.0));
        let p = parse_expression("(1 + 2) * 3").unwrap();
        assert!(matches!(p.kind, ExprKind::Binary { op: BinOp::Mul, .. }));
    }
}
