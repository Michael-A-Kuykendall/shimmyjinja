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
            // Lookahead for termination conditions
            if let Some(Token::BlockStart) = self.peek(0) {
                if let Some(Token::EndFor | Token::EndIf | Token::Else | Token::Elif) = self.peek(1) {
                    // Block terminator found; stop parsing this sequence
                    break;
                }
            }
            // Also stop deeply if EOF
            if self.peek(0).is_none() {
                break;
            }

            // Normal processing
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
                        Some(Token::If) => nodes.push(self.parse_if()?),
                        Some(t) => return Err(format!("Unexpected tag inside block: {:?}", t)),
                        None => return Err("Unexpected EOF inside block start".to_string()),
                    }
                }
                _ => break, // Should be unreachable given peek checks?
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
        let iterable = match self.consume() {
            Some(Token::Ident(s)) => s,
            t => {
                return Err(format!(
                    "Expected identifier for loop iterable, got {:?}",
                    t
                ))
            }
        };
        self.expect(Token::BlockEnd)?;

        let body = self.parse()?; // Recursively parse body

        // Expect endfor
        self.expect(Token::BlockStart)?;
        self.expect(Token::EndFor)?;
        self.expect(Token::BlockEnd)?;

        Ok(Node::For {
            target,
            iterable,
            body,
        })
    }

    fn parse_if(&mut self) -> Result<Node, String> {
        self.expect(Token::If)?;
        let condition = self.parse_expr()?;
        self.expect(Token::BlockEnd)?;

        let body = self.parse()?;
        let mut cases = vec![(condition, body)];
        let mut else_body = None;

        loop {
            // Check what comes next: {% elif ... %} or {% else %} or {% endif %}
            match self.peek(0) {
                Some(Token::BlockStart) => {
                    match self.peek(1) {
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
                            // After else, we must see endif
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
                        // If we hit EndFor here, it means we are missing EndIf?
                        // "Expected elif, else, or endif".
                        // Just fallback to returning error or break?
                        // If EndFor, then the `if` block is unterminated inside the `for`.
                        t => return Err(format!("Expected elif, else, or endif, got {:?}", t)),
                    }
                }
                None => return Err("Unexpected EOF parsing if block".to_string()),
                _ => {
                    return Err(format!(
                        "Expected tag start for control flow, got {:?}",
                        self.peek(0)
                    ))
                }
            }
        }

        Ok(Node::If { cases, else_body })
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
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
        let mut lhs = self.parse_eq()?;
        while let Some(Token::And) = self.peek(0) {
            self.consume();
            let rhs = self.parse_eq()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::And, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_eq(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_add()?;
        while let Some(Token::EqEq) = self.peek(0) {
            self.consume();
            let rhs = self.parse_add()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Eq, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_primary()?;
        while let Some(Token::Plus) = self.peek(0) {
            self.consume();
            let rhs = self.parse_primary()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Add, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let mut expr = match self.consume() {
            Some(Token::StringLit(s)) => Expr::StringLit(s),
            Some(Token::True) => Expr::BoolLit(true),
            Some(Token::False) => Expr::BoolLit(false),
            Some(Token::Ident(s)) => Expr::Var(s),
            Some(Token::LParen) => {
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                e
            }
            t => return Err(format!("Expected expression, got {:?}", t)),
        };

        // Handle suffixes: .attr, ['key']
        loop {
            match self.peek(0) {
                Some(Token::Dot) => {
                    self.consume(); // .
                    match self.consume() {
                        Some(Token::Ident(attr)) => {
                            expr = Expr::Attribute(Box::new(expr), attr);
                        }
                        t => return Err(format!("Expected identifier after dot, got {:?}", t)),
                    }
                }
                Some(Token::LBracket) => {
                    self.consume(); // [
                    let idx = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                }
                _ => break,
            }
        }

        Ok(expr)
    }
}
