use crate::dax::ast::{BinaryOp, Expr, UnaryOp};
use crate::dax::lexer::{tokenize, Token, TokenKind};

pub fn parse_dax(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, idx: 0 };
    let expr = parser.parse_expr()?;
    if !matches!(parser.peek(), TokenKind::Eof) {
        return Err(format!("Token inesperado após expressão: {:?}", parser.peek()));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
}

impl Parser {
    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.idx)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn advance(&mut self) -> TokenKind {
        let tok = self.peek().clone();
        if self.idx < self.tokens.len() {
            self.idx += 1;
        }
        tok
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), TokenKind::Var) {
            self.parse_var_block()
        } else {
            self.parse_or()
        }
    }

    fn parse_var_block(&mut self) -> Result<Expr, String> {
        let mut bindings = Vec::new();
        while matches!(self.peek(), TokenKind::Var) {
            self.advance();
            let name = match self.advance() {
                TokenKind::Ident(s) => s,
                other => return Err(format!("Esperado nome da variável após VAR, encontrado {:?}", other)),
            };
            if !matches!(self.advance(), TokenKind::Eq) {
                return Err(format!("Esperado '=' após VAR {}", name));
            }
            let val_expr = self.parse_or()?;
            bindings.push((name, val_expr));
        }

        if !matches!(self.advance(), TokenKind::Return) {
            return Err("Esperado RETURN após bloco VAR".to_string());
        }
        let body = self.parse_expr()?;
        Ok(Expr::VarBlock {
            bindings,
            body: Box::new(body),
        })
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), TokenKind::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison()?;
        while matches!(self.peek(), TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_concat()?;
        loop {
            match self.peek() {
                TokenKind::Eq => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Eq, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::StrictEq => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::StrictEq, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Neq => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Neq, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Lt => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Lt, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Lte => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Lte, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Gt => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Gt, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Gte => {
                    self.advance();
                    let right = self.parse_concat()?;
                    left = Expr::Binary { op: BinaryOp::Gte, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::In => {
                    self.advance();
                    let list = self.parse_braced_list()?;
                    left = Expr::InList { expr: Box::new(left), list };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_braced_list(&mut self) -> Result<Vec<Expr>, String> {
        if !matches!(self.advance(), TokenKind::LBrace) {
            return Err("Esperado '{' após operador IN".to_string());
        }
        let mut items = Vec::new();
        if !matches!(self.peek(), TokenKind::RBrace) {
            loop {
                items.push(self.parse_expr()?);
                if matches!(self.peek(), TokenKind::Comma | TokenKind::Semicolon) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        if !matches!(self.advance(), TokenKind::RBrace) {
            return Err("Esperado '}' ao final da lista IN".to_string());
        }
        Ok(items)
    }

    fn parse_concat(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        while matches!(self.peek(), TokenKind::Amp) {
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                op: BinaryOp::Concat,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                TokenKind::Plus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Binary { op: BinaryOp::Add, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Minus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::Binary { op: BinaryOp::Sub, left: Box::new(left), right: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                TokenKind::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Binary { op: BinaryOp::Mul, left: Box::new(left), right: Box::new(right) };
                }
                TokenKind::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Binary { op: BinaryOp::Div, left: Box::new(left), right: Box::new(right) };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            TokenKind::Minus => {
                self.advance();
                Ok(Expr::Unary { op: UnaryOp::Neg, expr: Box::new(self.parse_unary()?) })
            }
            TokenKind::Plus => {
                self.advance();
                Ok(Expr::Unary { op: UnaryOp::Pos, expr: Box::new(self.parse_unary()?) })
            }
            TokenKind::Not => {
                self.advance();
                Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(self.parse_unary()?) })
            }
            _ => self.parse_primary(),
        }
    }

    /// TRUE/FALSE may appear bare or as zero-arg calls TRUE()/FALSE().
    fn consume_optional_zero_arg_parens(&mut self, name: &str) -> Result<(), String> {
        if matches!(self.peek(), TokenKind::LParen) {
            self.advance();
            if !matches!(self.advance(), TokenKind::RParen) {
                return Err(format!("Esperado ')' após {}(", name));
            }
        }
        Ok(())
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            TokenKind::Number(n) => Ok(Expr::Number(n)),
            TokenKind::String(s) => Ok(Expr::Text(s)),
            TokenKind::True => {
                self.consume_optional_zero_arg_parens("TRUE")?;
                Ok(Expr::Boolean(true))
            }
            TokenKind::False => {
                self.consume_optional_zero_arg_parens("FALSE")?;
                Ok(Expr::Boolean(false))
            }
            TokenKind::Bracket(b) => Ok(Expr::BracketRef(b)),
            TokenKind::LParen => {
                let e = self.parse_expr()?;
                if !matches!(self.advance(), TokenKind::RParen) {
                    return Err("Parêntese ')' esperado".to_string());
                }
                Ok(e)
            }
            TokenKind::Ident(id) => {
                if matches!(self.peek(), TokenKind::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    if !matches!(self.peek(), TokenKind::RParen) {
                        loop {
                            // Omitted positional argument, e.g. RANKX(t, e, , ASC)
                            if matches!(self.peek(), TokenKind::Comma | TokenKind::Semicolon | TokenKind::RParen) {
                                args.push(Expr::Blank);
                            } else {
                                args.push(self.parse_expr()?);
                            }
                            if matches!(self.peek(), TokenKind::Comma | TokenKind::Semicolon) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    if !matches!(self.advance(), TokenKind::RParen) {
                        return Err(format!("Esperado ')' após argumentos de {}", id));
                    }
                    Ok(Expr::Call { name: id, args })
                } else if matches!(self.peek(), TokenKind::Bracket(_)) {
                    let TokenKind::Bracket(col) = self.advance() else {
                        unreachable!();
                    };
                    Ok(Expr::TableColRef {
                        table: id,
                        column: col,
                    })
                } else {
                    Ok(Expr::Ident(id))
                }
            }
            other => Err(format!("Expressão inesperada: {:?}", other)),
        }
    }
}
