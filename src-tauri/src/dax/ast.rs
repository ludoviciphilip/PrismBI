#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Concat,
    Eq,
    StrictEq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
    Ident(String),
    BracketRef(String),
    TableColRef { table: String, column: String },
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    InList { expr: Box<Expr>, list: Vec<Expr> },
    Call { name: String, args: Vec<Expr> },
    VarBlock { bindings: Vec<(String, Expr)>, body: Box<Expr> },
}
