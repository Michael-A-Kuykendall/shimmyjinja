use crate::ast::*;
use crate::lexer::{Token, Tokenizer};
use std::collections::VecDeque;

pub struct Parser<'a> {
    lexer: Tokenizer<'a>,
    buffer: VecDeque<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lexer: Tokenizer::new(input),
            buffer: VecDeque::new(),
        }
    }

    fn peek(&mut self, n: usize) -> Option<&Token> {
        while self.buffer.len() <= n {
            if let Some(token) = self.lexer.next_token() {
                self.buffer.push_back(token);
            } else {
                return None;
            }
        }
        self.buffer.get(n)
    }

    fn consume(&mut self) -> Option<Token> {
        if self.buffer.is_empty() {
            self.lexer.next_token()
        } else {
            self.buffer.pop_front()
        }
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        match self.consume() {
            Some(t) if t == token => Ok(()),
            Some(t) => Err(format!("Expected {:?}, got {:?}", token, t)),
            None => Err(format!("Expected {:?}, got EOF", token)),
        }
    }

    pub fn parse(&mut self) -> Result<Template, String> {
        let mut nodes = Vec::new();
        loop {
            // Stop at block terminators (endfor, endif, else, elif)
            if let Some(Token::BlockStart) = self.peek(0) {
                if let Some(Token::EndFor | Token::EndIf | Token::Else | Token::Elif) =
                    self.peek(1)
                {
                    break;
                }
            }
            if self.peek(0).is_none() {
                break;
            }

            match self.peek(0).cloned() {
                Some(Token::Text(s)) => {
                    self.consume();
                    nodes.push(Node::Text(s));
                }
                Some(Token::VarStart) => {
                    self.consume(); // {{
                    let expr = self.parse_expr()?;
                    self.expect(Token::VarEnd)?;
                    nodes.push(Node::Var(expr));
                }
                Some(Token::BlockStart) => {
                    self.consume(); // {%
                    match self.peek(0) {
                        Some(Token::For) => nodes.push(self.parse_for()?),
                        Some(Token::If)  => nodes.push(self.parse_if()?),
                        Some(Token::Set) => nodes.push(self.parse_set()?),
                        Some(t) => {
                            let t = t.clone();
                            return Err(format!("Unexpected tag inside block: {:?}", t));
                        }
                        None => return Err("Unexpected EOF inside block start".to_string()),
                    }
                }
                _ => break,
            }
        }
        Ok(nodes)
    }

    fn parse_for(&mut self) -> Result<Node, String> {
        self.expect(Token::For)?;
        let target = match self.consume() {
            Some(Token::Ident(s)) => s,
            t => return Err(format!("Expected identifier for loop target, got {:?}", t)),
        };
        self.expect(Token::In)?;
        let iterable = self.parse_expr()?;
        self.expect(Token::BlockEnd)?;

        let body = self.parse()?;

        self.expect(Token::BlockStart)?;
        self.expect(Token::EndFor)?;
        self.expect(Token::BlockEnd)?;

        Ok(Node::For { target, iterable, body })
    }

    fn parse_if(&mut self) -> Result<Node, String> {
        self.expect(Token::If)?;
        let condition = self.parse_expr()?;
        self.expect(Token::BlockEnd)?;

        let body = self.parse()?;
        let mut cases = vec![(condition, body)];
        let mut else_body = None;

        loop {
            match self.peek(0) {
                Some(Token::BlockStart) => match self.peek(1) {
                    Some(Token::Elif) => {
                        self.consume(); // {%
                        self.consume(); // elif
                        let cond = self.parse_expr()?;
                        self.expect(Token::BlockEnd)?;
                        let block = self.parse()?;
                        cases.push((cond, block));
                    }
                    Some(Token::Else) => {
                        self.consume(); // {%
                        self.consume(); // else
                        self.expect(Token::BlockEnd)?;
                        else_body = Some(self.parse()?);
                        self.expect(Token::BlockStart)?;
                        self.expect(Token::EndIf)?;
                        self.expect(Token::BlockEnd)?;
                        break;
                    }
                    Some(Token::EndIf) => {
                        self.consume(); // {%
                        self.consume(); // endif
                        self.expect(Token::BlockEnd)?;
                        break;
                    }
                    t => return Err(format!("Expected elif, else, or endif, got {:?}", t)),
                },
                None => return Err("Unexpected EOF parsing if block".to_string()),
                t => {
                    return Err(format!(
                        "Expected tag start for control flow, got {:?}",
                        t
                    ))
                }
            }
        }

        Ok(Node::If { cases, else_body })
    }

    fn parse_set(&mut self) -> Result<Node, String> {
        self.expect(Token::Set)?;
        let base = match self.consume() {
            Some(Token::Ident(s)) => s,
            t => return Err(format!("Expected identifier after 'set', got {:?}", t)),
        };
        // Handle dotted assignment: ns.foo = expr
        // Parsed as flat key "ns.foo" — attribute gets discarded in eval (no-op for namespace).
        let name = if let Some(Token::Dot) = self.peek(0) {
            let mut parts = vec![base];
            while let Some(Token::Dot) = self.peek(0) {
                self.consume(); // .
                match self.consume() {
                    Some(Token::Ident(s)) => parts.push(s),
                    t => return Err(format!("Expected ident after '.' in set, got {:?}", t)),
                }
            }
            parts.join(".")
        } else {
            base
        };
        self.expect(Token::Assign)?;
        let expr = self.parse_expr()?;
        self.expect(Token::BlockEnd)?;
        Ok(Node::Set { name, expr })
    }

    // ── Expression grammar (lowest to highest precedence) ──────────────────
    //
    //  expr         = or_expr
    //  or_expr      = and_expr  ('or'  and_expr)*
    //  and_expr     = not_expr  ('and' not_expr)*
    //  not_expr     = 'not' not_expr  |  compare_expr
    //  compare_expr = add_expr  (('==' | '!=' | 'is' ['not']) add_expr)*
    //  add_expr     = mul_expr  ('+' mul_expr)*
    //  mul_expr     = postfix   ('%' postfix)*
    //  postfix      = base  ('.' IDENT | '[' (expr | slice) ']' | '|' IDENT ['(' args ')'])*
    //  base         = STRING | INT | BOOL | IDENT ['(' args ')'] | '(' expr ')' | '-' INT

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let val = self.parse_or()?;
        // Inline ternary: `val if cond else fallback`
        if let Some(Token::If) = self.peek(0) {
            self.consume(); // if
            let cond = self.parse_or()?;
            let else_val = if let Some(Token::Else) = self.peek(0) {
                self.consume(); // else
                self.parse_or()?
            } else {
                Expr::StringLit(String::new()) // implicit empty string when no else
            };
            Ok(Expr::Ternary(Box::new(cond), Box::new(val), Box::new(else_val)))
        } else {
            Ok(val)
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_and()?;
        while let Some(Token::Or) = self.peek(0) {
            self.consume();
            let rhs = self.parse_and()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Or, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_not()?;
        while let Some(Token::And) = self.peek(0) {
            self.consume();
            let rhs = self.parse_not()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::And, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_not(&mut self) -> Result<Expr, String> {
        if let Some(Token::Not) = self.peek(0) {
            self.consume();
            let inner = self.parse_not()?; // right-associative
            Ok(Expr::Not(Box::new(inner)))
        } else {
            self.parse_compare()
        }
    }

    fn parse_compare(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_add()?;
        loop {
            // Check for 'not in' compound operator before the main match to avoid
            // borrowing self twice at the same time (peek(0) + peek(1)).
            if matches!(self.peek(0), Some(Token::Not))
                && matches!(self.peek(1), Some(Token::In))
            {
                self.consume(); // not
                self.consume(); // in
                let rhs = self.parse_add()?;
                lhs = Expr::BinOp(Box::new(lhs), BinOp::NotIn, Box::new(rhs));
                continue;
            }
            match self.peek(0) {
                Some(Token::EqEq) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Eq, Box::new(rhs));
                }
                Some(Token::Ne) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Ne, Box::new(rhs));
                }
                Some(Token::In) => {
                    self.consume(); // in
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::In, Box::new(rhs));
                }
                Some(Token::Is) => {
                    self.consume(); // is
                    let negated = if let Some(Token::Not) = self.peek(0) {
                        self.consume(); // not
                        true
                    } else {
                        false
                    };
                    // Accept both Ident and keyword tokens as test names (e.g., `is false`, `is true`)
                    let test_name = match self.consume() {
                        Some(Token::Ident(s)) => s,
                        Some(Token::False)    => "false".to_string(),
                        Some(Token::True)     => "true".to_string(),
                        t => return Err(format!("Expected test name after 'is', got {:?}", t)),
                    };
                    lhs = Expr::IsTest(Box::new(lhs), negated, test_name);
                }
                Some(Token::Lt) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Lt, Box::new(rhs));
                }
                Some(Token::Gt) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Gt, Box::new(rhs));
                }
                Some(Token::Le) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Le, Box::new(rhs));
                }
                Some(Token::Ge) => {
                    self.consume();
                    let rhs = self.parse_add()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Ge, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        loop {
            match self.peek(0) {
                Some(Token::Plus) => {
                    self.consume();
                    let rhs = self.parse_mul()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Add, Box::new(rhs));
                }
                Some(Token::Minus) => {
                    self.consume();
                    let rhs = self.parse_mul()?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::Sub, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_postfix()?;
        while let Some(Token::Percent) = self.peek(0) {
            self.consume();
            let rhs = self.parse_postfix()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Mod, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_base()?;
        loop {
            match self.peek(0) {
                Some(Token::Dot) => {
                    self.consume();
                    match self.consume() {
                        Some(Token::Ident(attr)) => {
                            // If followed by `(`, this is a method call: obj.method(args)
                            // Treat as a filter for eval purposes.
                            if let Some(Token::LParen) = self.peek(0) {
                                self.consume(); // (
                                let args = self.parse_args()?;
                                self.expect(Token::RParen)?;
                                expr = Expr::Filter(Box::new(expr), attr, args);
                            } else {
                                expr = Expr::Attribute(Box::new(expr), attr);
                            }
                        }
                        t => return Err(format!("Expected identifier after '.', got {:?}", t)),
                    }
                }
                Some(Token::LBracket) => {
                    self.consume(); // [
                    // Check for slice: [:end], [start:], [start:end], vs plain [idx]
                    if let Some(Token::Colon) = self.peek(0) {
                        // [:end] or [:]  — start is None
                        self.consume(); // :
                        let end = if let Some(Token::RBracket) = self.peek(0) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        self.expect(Token::RBracket)?;
                        expr = Expr::Slice(Box::new(expr), None, end);
                    } else {
                        let idx = self.parse_expr()?;
                        if let Some(Token::Colon) = self.peek(0) {
                            // [start:] or [start:end]
                            self.consume(); // :
                            let end = if let Some(Token::RBracket) = self.peek(0) {
                                None
                            } else {
                                Some(Box::new(self.parse_expr()?))
                            };
                            self.expect(Token::RBracket)?;
                            expr = Expr::Slice(Box::new(expr), Some(Box::new(idx)), end);
                        } else {
                            self.expect(Token::RBracket)?;
                            expr = Expr::Index(Box::new(expr), Box::new(idx));
                        }
                    }
                }
                Some(Token::Pipe) => {
                    self.consume(); // |
                    let filter_name = match self.consume() {
                        Some(Token::Ident(s)) => s,
                        t => {
                            return Err(format!("Expected filter name after '|', got {:?}", t))
                        }
                    };
                    let args = if let Some(Token::LParen) = self.peek(0) {
                        self.consume(); // (
                        let a = self.parse_args()?;
                        self.expect(Token::RParen)?;
                        a
                    } else {
                        Vec::new()
                    };
                    expr = Expr::Filter(Box::new(expr), filter_name, args);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_base(&mut self) -> Result<Expr, String> {
        match self.consume() {
            Some(Token::StringLit(s)) => Ok(Expr::StringLit(s)),
            Some(Token::IntLit(n))    => Ok(Expr::IntLit(n)),
            Some(Token::Minus) => {
                // Unary minus — only meaningful before an integer literal
                match self.consume() {
                    Some(Token::IntLit(n)) => Ok(Expr::IntLit(-n)),
                    t => Err(format!("Expected integer after unary '-', got {:?}", t)),
                }
            }
            Some(Token::True)  => Ok(Expr::BoolLit(true)),
            Some(Token::False) => Ok(Expr::BoolLit(false)),
            Some(Token::Ident(s)) => {
                // Function call: ident(args)
                if let Some(Token::LParen) = self.peek(0) {
                    self.consume(); // (
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(s, args))
                } else {
                    Ok(Expr::Var(s))
                }
            }
            Some(Token::LParen) => {
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            t => Err(format!("Expected expression, got {:?}", t)),
        }
    }

    /// Parse a comma-separated argument list (stops before `)`).
    /// Handles keyword arguments `name=value` by discarding the key and keeping the value.
    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if let Some(Token::RParen) = self.peek(0) {
            return Ok(args);
        }
        loop {
            // Keyword argument: ident = expr  → discard key, keep value
            if matches!(self.peek(0), Some(Token::Ident(_)))
                && matches!(self.peek(1), Some(Token::Assign))
            {
                self.consume(); // key name
                self.consume(); // =
            }
            args.push(self.parse_expr()?);
            if let Some(Token::Comma) = self.peek(0) {
                self.consume(); // ,
                if let Some(Token::RParen) = self.peek(0) {
                    break; // trailing comma
                }
            } else {
                break;
            }
        }
        Ok(args)
    }
}
