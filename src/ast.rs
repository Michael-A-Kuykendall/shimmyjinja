#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Eq,
    Add,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLit(String),
    BoolLit(bool),
    Var(String),
    Attribute(Box<Expr>, String), // foo.bar
    Index(Box<Expr>, Box<Expr>),  // foo['bar']
    BinOp(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text(String),
    Var(Expr),
    For {
        target: String,   // e.g., "message"
        iterable: String, // e.g., "messages"
        body: Vec<Node>,
    },
    If {
        cases: Vec<(Expr, Vec<Node>)>, // (condition, body). Includes if and elifs.
        else_body: Option<Vec<Node>>,
    },
}

pub type Template = Vec<Node>;
