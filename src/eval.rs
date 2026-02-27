use crate::ast::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Bool(bool),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Null,
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Null => false,
        }
    }
}

pub struct Evaluator {
    // We treat context as a stack of scopes ideally, but for simple Jinja
    // we can just use a single map and clone/insert/remove carefully,
    // or use a `Vec<HashMap>` stack. Standard Jinja has scoping.
    scopes: Vec<HashMap<String, Value>>,
}

impl Evaluator {
    pub fn new(context: HashMap<String, Value>) -> Self {
        Self {
            scopes: vec![context],
        }
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn set_local(&mut self, name: String, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    pub fn render(&mut self, template: &Template) -> Result<String, String> {
        let mut output = String::new();
        for node in template {
            match node {
                Node::Text(s) => output.push_str(s),
                Node::Var(expr) => {
                    let val = self.eval_expr(expr)?;
                    match val {
                        Value::String(s) => output.push_str(&s),
                        Value::Bool(b) => output.push_str(&b.to_string()),
                        Value::Null => {} // Print nothing for null? or "None"? Jinja prints nothing usually.
                        _ => return Err(format!("Cannot render complex type {:?}", val)),
                    }
                }
                Node::For {
                    target,
                    iterable,
                    body,
                } => {
                    let iter_val = self.eval_expr(&Expr::Var(iterable.clone()))?;
                    match iter_val {
                        Value::Array(items) => {
                            let len = items.len();
                            for (i, item) in items.into_iter().enumerate() {
                                self.push_scope();
                                self.set_local(target.clone(), item);

                                // Set loop variable
                                let mut loop_map = HashMap::new();
                                loop_map.insert("index0".to_string(), Value::String(i.to_string())); // Hacky: keeping numerics as strings if we don't have int?
                                                                                                     // Stick to logic: we need 'last'.
                                loop_map.insert("last".to_string(), Value::Bool(i == len - 1));
                                loop_map.insert("first".to_string(), Value::Bool(i == 0));
                                self.set_local("loop".to_string(), Value::Map(loop_map));

                                output.push_str(&self.render(body)?);
                                self.pop_scope();
                            }
                        }
                        Value::Null => {} // Missing iterable = skip loop (Jinja behavior)
                        _ => return Err(format!("Expected array for loop, got {:?}", iter_val)),
                    }
                }
                Node::If { cases, else_body } => {
                    let mut matched = false;
                    for (cond, body) in cases {
                        let val = self.eval_expr(cond)?;
                        if val.is_truthy() {
                            output.push_str(&self.render(body)?);
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(body) = else_body {
                            output.push_str(&self.render(body)?);
                        }
                    }
                }
            }
        }
        Ok(output)
    }

    fn eval_expr(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::Var(name) => Ok(self
                .get_var(name)
                .unwrap_or(Value::Null)),
            Expr::Attribute(obj, attr) => {
                let val = self.eval_expr(obj)?;
                match val {
                    Value::Map(m) => m
                        .get(attr)
                        .cloned()
                        .ok_or_else(|| format!("Attribute {} not found", attr)),
                    // Handle loop.last access
                    _ => Err(format!(
                        "Cannot get attribute {} of non-map {:?}",
                        attr, val
                    )),
                }
            }
            Expr::Index(obj, idx) => {
                let val = self.eval_expr(obj)?;
                let idx_val = self.eval_expr(idx)?;
                match (val, idx_val) {
                    (Value::Map(m), Value::String(s)) => {
                        m.get(&s).cloned().ok_or(format!("Key {} not found", s))
                    }
                    (Value::Array(a), Value::String(s)) => {
                        // Maybe integer parse?
                        if let Ok(i) = s.parse::<usize>() {
                            a.get(i)
                                .cloned()
                                .ok_or(format!("Index {} out of bounds", i))
                        } else {
                            Err(format!("Index must be integer, got {}", s))
                        }
                    }
                    _ => Err("Invalid index access".to_string()),
                }
            }
            Expr::BinOp(curr_lhs, op, curr_rhs) => {
                let l = self.eval_expr(curr_lhs)?;
                let r = self.eval_expr(curr_rhs)?;
                match op {
                    BinOp::Eq => Ok(Value::Bool(l == r)),
                    BinOp::Add => match (l, r) {
                        (Value::String(s1), Value::String(s2)) => Ok(Value::String(s1 + &s2)),
                        _ => Err("Add only supports strings".to_string()),
                    },
                    BinOp::And => Ok(Value::Bool(l.is_truthy() && r.is_truthy())),
                    BinOp::Or => Ok(Value::Bool(l.is_truthy() || r.is_truthy())),
                }
            }
        }
    }
}
