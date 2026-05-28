use crate::ast::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Bool(bool),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Null,
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b)   => *b,
            Value::Int(n)    => *n != 0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a)  => !a.is_empty(),
            Value::Map(m)    => !m.is_empty(),
            Value::Null      => false,
        }
    }
}

pub struct Evaluator {
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
                        Value::Int(n)    => output.push_str(&n.to_string()),
                        Value::Bool(b)   => output.push_str(if b { "True" } else { "False" }),
                        Value::Null      => {} // Jinja2 renders None/null as empty
                        _ => return Err(format!("Cannot render complex type {:?}", val)),
                    }
                }
                Node::For { target, iterable, body } => {
                    let iter_val = self.eval_expr(iterable)?;
                    match iter_val {
                        Value::Array(items) => {
                            let len = items.len();
                            for (i, item) in items.into_iter().enumerate() {
                                self.push_scope();
                                self.set_local(target.clone(), item);

                                // Inject loop.* variables
                                let mut loop_map = HashMap::new();
                                loop_map.insert("index0".to_string(), Value::Int(i as i64));
                                loop_map.insert("index".to_string(),  Value::Int(i as i64 + 1));
                                loop_map.insert("first".to_string(),  Value::Bool(i == 0));
                                loop_map.insert("last".to_string(),   Value::Bool(i == len - 1));
                                self.set_local("loop".to_string(), Value::Map(loop_map));

                                output.push_str(&self.render(body)?);
                                self.pop_scope();
                            }
                        }
                        Value::Null => {} // Missing iterable = skip loop (Jinja2 behavior)
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
                Node::Set { name, expr } => {
                    // {% set name = expr %} — assigns into the current scope.
                    // If blocks don't push scopes, so this correctly modifies
                    // the enclosing for-loop scope (or root scope) as Jinja2 does.
                    let val = self.eval_expr(expr)?;
                    self.set_local(name.clone(), val);
                }
            }
        }
        Ok(output)
    }

    fn eval_expr(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::Var(name)    => Ok(self.get_var(name).unwrap_or(Value::Null)),

            Expr::Not(inner) => {
                let val = self.eval_expr(inner)?;
                Ok(Value::Bool(!val.is_truthy()))
            }

            Expr::Attribute(obj, attr) => {
                let val = self.eval_expr(obj)?;
                match val {
                    Value::Map(m) => Ok(m.get(attr).cloned().unwrap_or(Value::Null)),
                    // Graceful degradation: attribute access on non-map returns Null
                    _ => Ok(Value::Null),
                }
            }

            Expr::Index(obj, idx) => {
                let val     = self.eval_expr(obj)?;
                let idx_val = self.eval_expr(idx)?;
                match (val, idx_val) {
                    // Map key access: map['key']
                    (Value::Map(m), Value::String(s)) => {
                        Ok(m.get(&s).cloned().unwrap_or(Value::Null))
                    }
                    // Array access with integer (including negative)
                    (Value::Array(a), Value::Int(i)) => {
                        let len = a.len() as i64;
                        let idx = if i < 0 { len + i } else { i };
                        if idx < 0 || idx >= len {
                            Err(format!("Index {} out of bounds (len={})", i, len))
                        } else {
                            Ok(a[idx as usize].clone())
                        }
                    }
                    // Array access with string that parses as integer
                    (Value::Array(a), Value::String(s)) => {
                        if let Ok(i) = s.parse::<usize>() {
                            a.get(i)
                                .cloned()
                                .ok_or_else(|| format!("Index {} out of bounds", i))
                        } else {
                            Err(format!("Array index must be integer, got '{}'", s))
                        }
                    }
                    (v, i) => Err(format!("Invalid index access: {:?}[{:?}]", v, i)),
                }
            }

            Expr::BinOp(lhs_expr, op, rhs_expr) => {
                let l = self.eval_expr(lhs_expr)?;
                let r = self.eval_expr(rhs_expr)?;
                match op {
                    BinOp::Eq  => Ok(Value::Bool(l == r)),
                    BinOp::Ne  => Ok(Value::Bool(l != r)),
                    BinOp::And => Ok(Value::Bool(l.is_truthy() && r.is_truthy())),
                    BinOp::Or  => Ok(Value::Bool(l.is_truthy() || r.is_truthy())),
                    BinOp::Lt => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
                        _ => Ok(Value::Bool(false)),
                    },
                    BinOp::Gt => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
                        _ => Ok(Value::Bool(false)),
                    },
                    BinOp::Le => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
                        _ => Ok(Value::Bool(false)),
                    },
                    BinOp::Ge => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
                        _ => Ok(Value::Bool(false)),
                    },
                    BinOp::Add => match (l, r) {
                        (Value::String(s1), Value::String(s2)) => Ok(Value::String(s1 + &s2)),
                        (Value::Int(a), Value::Int(b))         => Ok(Value::Int(a + b)),
                        (l, r) => Err(format!("'+' unsupported for {:?} and {:?}", l, r)),
                    },
                    BinOp::Sub => match (l, r) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
                        (l, r) => Err(format!("'-' unsupported for {:?} and {:?}", l, r)),
                    },
                    BinOp::Mod => match (l, r) {
                        (Value::Int(a), Value::Int(b)) if b != 0 => Ok(Value::Int(a % b)),
                        (Value::Int(_), Value::Int(0)) => Err("Modulo by zero".to_string()),
                        (l, r) => Err(format!("'%' unsupported for {:?} and {:?}", l, r)),
                    },
                    BinOp::In => match (l, r) {
                        (Value::String(key), Value::Map(m))      => Ok(Value::Bool(m.contains_key(&key))),
                        (val, Value::Array(a))                   => Ok(Value::Bool(a.contains(&val))),
                        (Value::String(needle), Value::String(h)) => Ok(Value::Bool(h.contains(needle.as_str()))),
                        _ => Ok(Value::Bool(false)),
                    },
                    BinOp::NotIn => match (l, r) {
                        (Value::String(key), Value::Map(m))      => Ok(Value::Bool(!m.contains_key(&key))),
                        (val, Value::Array(a))                   => Ok(Value::Bool(!a.contains(&val))),
                        (Value::String(needle), Value::String(h)) => Ok(Value::Bool(!h.contains(needle.as_str()))),
                        _ => Ok(Value::Bool(true)),
                    },
                }
            }

            Expr::Filter(inner, name, args) => {
                let val = self.eval_expr(inner)?;
                match name.as_str() {
                    "trim" => match val {
                        Value::String(s) => Ok(Value::String(s.trim().to_string())),
                        other => Ok(other),
                    },
                    "default" | "d" => {
                        let is_falsy = matches!(&val, Value::Null)
                            || matches!(&val, Value::String(s) if s.is_empty());
                        if is_falsy {
                            if let Some(default_expr) = args.first() {
                                self.eval_expr(default_expr)
                            } else {
                                Ok(Value::String(String::new()))
                            }
                        } else {
                            Ok(val)
                        }
                    }
                    "upper" => match val {
                        Value::String(s) => Ok(Value::String(s.to_uppercase())),
                        other => Ok(other),
                    },
                    "lower" => match val {
                        Value::String(s) => Ok(Value::String(s.to_lowercase())),
                        other => Ok(other),
                    },
                    "length" | "count" => match &val {
                        Value::String(s)  => Ok(Value::Int(s.len() as i64)),
                        Value::Array(a)   => Ok(Value::Int(a.len() as i64)),
                        _ => Ok(Value::Int(0)),
                    },
                    // Unknown filter: return value unchanged (graceful degradation)
                    _ => Ok(val),
                }
            }

            Expr::Call(func_name, _args) => {
                match func_name.as_str() {
                    // raise_exception(...) is a Jinja2 macro used in some templates as a
                    // guard. We treat it as a no-op (return empty string) so that the
                    // rest of the template renders correctly.
                    "raise_exception" => Ok(Value::String(String::new())),
                    // namespace() returns an empty Map (Jinja2 scoped namespace object)
                    "namespace" => Ok(Value::Map(HashMap::new())),
                    // Unknown function calls return Null (renders as empty)
                    _ => Ok(Value::Null),
                }
            }

            Expr::Ternary(cond, then_val, else_val) => {
                let c = self.eval_expr(cond)?;
                if c.is_truthy() {
                    self.eval_expr(then_val)
                } else {
                    self.eval_expr(else_val)
                }
            }

            Expr::IsTest(inner, negated, test_name) => {
                let val = self.eval_expr(inner)?;
                let result = match test_name.as_str() {
                    "defined"         => !matches!(val, Value::Null),
                    "undefined"       =>  matches!(val, Value::Null),
                    "none" | "None"   =>  matches!(val, Value::Null),
                    "string"          =>  matches!(val, Value::String(_)),
                    "integer" | "number" => matches!(val, Value::Int(_)),
                    "boolean"         =>  matches!(val, Value::Bool(_)),
                    "iterable" | "sequence" => matches!(val, Value::Array(_) | Value::String(_)),
                    "mapping"         =>  matches!(val, Value::Map(_)),
                    "true"            =>  val.is_truthy(),
                    "false"           => !val.is_truthy(),
                    // Unknown test name — safe false (graceful degradation)
                    _                 => false,
                };
                Ok(Value::Bool(if *negated { !result } else { result }))
            }

            Expr::Slice(obj_expr, start_expr, end_expr) => {
                let obj = self.eval_expr(obj_expr)?;
                match obj {
                    Value::Array(a) => {
                        let len = a.len() as i64;
                        let start = match start_expr {
                            Some(e) => match self.eval_expr(e)? {
                                Value::Int(n) => (if n < 0 { len + n } else { n }).clamp(0, len) as usize,
                                _ => 0,
                            },
                            None => 0,
                        };
                        let end = match end_expr {
                            Some(e) => match self.eval_expr(e)? {
                                Value::Int(n) => (if n < 0 { len + n } else { n }).clamp(0, len) as usize,
                                _ => len as usize,
                            },
                            None => len as usize,
                        };
                        Ok(Value::Array(a[start.min(a.len())..end.min(a.len())].to_vec()))
                    }
                    // Slicing a non-array is a no-op — return original value
                    other => Ok(other),
                }
            }
        }
    }
}
