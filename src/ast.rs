#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mod,
    And,
    Or,
    In,
    NotIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLit(String),
    IntLit(i64),
    BoolLit(bool),
    Var(String),
    Attribute(Box<Expr>, String),                             // foo.bar
    Index(Box<Expr>, Box<Expr>),                              // foo['bar'] or foo[0]
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),   // foo[start:end]
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    Not(Box<Expr>),                                           // not expr
    IsTest(Box<Expr>, bool, String),                          // expr is [not] test_name
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),                 // cond, then_val, else_val
    Filter(Box<Expr>, String, Vec<Expr>),                     // expr | filter_name(args)
    Call(String, Vec<Expr>),                                  // func_name(args)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text(String),
    Var(Expr),
    For {
        target: String,
        iterable: Expr,   // typically Var("messages") but supports any expr
        body: Vec<Node>,
    },
    If {
        cases: Vec<(Expr, Vec<Node>)>, // (condition, body). Includes if and elifs.
        else_body: Option<Vec<Node>>,
    },
    Set {
        name: String,
        expr: Expr,
    },
}

pub type Template = Vec<Node>;
