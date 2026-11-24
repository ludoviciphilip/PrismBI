#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(f64),
    String(String),
    Ident(String),
    Bracket(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Amp,
    Eq,
    StrictEq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    AndAnd,
    OrOr,
    Var,
    Return,
    In,
    Not,
    True,
    False,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        // Line comments: // or --
        if (ch == '/' && chars.get(i + 1) == Some(&'/'))
            || (ch == '-' && chars.get(i + 1) == Some(&'-'))
        {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comments: /* ... */
        if ch == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }

        let pos = i;

        match ch {
            '(' => { tokens.push(Token { kind: TokenKind::LParen, pos }); i += 1; }
            ')' => { tokens.push(Token { kind: TokenKind::RParen, pos }); i += 1; }
            '{' => { tokens.push(Token { kind: TokenKind::LBrace, pos }); i += 1; }
            '}' => { tokens.push(Token { kind: TokenKind::RBrace, pos }); i += 1; }
            ',' => { tokens.push(Token { kind: TokenKind::Comma, pos }); i += 1; }
            ';' => { tokens.push(Token { kind: TokenKind::Semicolon, pos }); i += 1; }
            '+' => { tokens.push(Token { kind: TokenKind::Plus, pos }); i += 1; }
            '-' => { tokens.push(Token { kind: TokenKind::Minus, pos }); i += 1; }
            '*' => { tokens.push(Token { kind: TokenKind::Star, pos }); i += 1; }
            '/' => { tokens.push(Token { kind: TokenKind::Slash, pos }); i += 1; }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    tokens.push(Token { kind: TokenKind::AndAnd, pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Amp, pos });
                    i += 1;
                }
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    tokens.push(Token { kind: TokenKind::OrOr, pos });
                    i += 2;
                } else {
                    return Err(format!("Token '|' inesperado na posição {}", i));
                }
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::StrictEq, pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Eq, pos });
                    i += 1;
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'>') {
                    tokens.push(Token { kind: TokenKind::Neq, pos });
                    i += 2;
                } else if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::Lte, pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Lt, pos });
                    i += 1;
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { kind: TokenKind::Gte, pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Gt, pos });
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() {
                    if chars[i] == '"' {
                        if chars.get(i + 1) == Some(&'"') {
                            s.push('"');
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
                tokens.push(Token { kind: TokenKind::String(s), pos });
            }
            '\'' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() {
                    if chars[i] == '\'' {
                        if chars.get(i + 1) == Some(&'\'') {
                            s.push('\'');
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
                tokens.push(Token { kind: TokenKind::Ident(s), pos });
            }
            '[' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != ']' {
                    s.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() && chars[i] == ']' {
                    i += 1;
                }
                tokens.push(Token { kind: TokenKind::Bracket(s.trim().to_string()), pos });
            }
            c if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())) => {
                let mut num_s = String::new();
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    num_s.push(chars[i]);
                    i += 1;
                }
                let val = num_s
                    .parse::<f64>()
                    .map_err(|_| format!("Número DAX inválido: {}", num_s))?;
                tokens.push(Token { kind: TokenKind::Number(val), pos });
            }
            c if is_ident_start(c) => {
                let mut id = String::new();
                while i < chars.len() && is_ident_continue(chars[i]) {
                    id.push(chars[i]);
                    i += 1;
                }
                let kind = match id.to_uppercase().as_str() {
                    "VAR" => TokenKind::Var,
                    "RETURN" => TokenKind::Return,
                    "IN" => TokenKind::In,
                    "NOT" => TokenKind::Not,
                    "TRUE" => TokenKind::True,
                    "FALSE" => TokenKind::False,
                    _ => TokenKind::Ident(id),
                };
                tokens.push(Token { kind, pos });
            }
            other => {
                return Err(format!("Caractere inesperado '{}' na posição {}", other, i));
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        pos: chars.len(),
    });
    Ok(tokens)
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}
