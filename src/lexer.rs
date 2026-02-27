#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text(String),
    BlockStart, // {%
    BlockEnd,   // %}
    VarStart,   // {{
    VarEnd,     // }}

    // Keywords
    If,
    Elif,
    Else,
    EndIf,
    For,
    In,
    EndFor,
    And,
    Or,
    True,
    False,

    // Symbols
    EqEq,     // ==
    Plus,     // +
    Dot,      // .
    LBracket, // [
    RBracket, // ]
    LParen,   // (
    RParen,   // )

    // Data
    Ident(String),
    StringLit(String),
}

#[derive(Clone)]
pub struct Tokenizer<'a> {
    input: &'a str,
    cursor: usize,
    in_tag: bool,
    trim_blocks: bool,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            cursor: 0,
            in_tag: false,
            trim_blocks: true,
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.cursor..]
    }

    fn advance(&mut self, n: usize) {
        self.cursor += n;
    }

    pub fn next_token(&mut self) -> Option<Token> {
        let rest = self.remaining();
        if rest.is_empty() {
            return None;
        }

        if !self.in_tag {
            // Find next `{{` or `{%`
            let next_tag = rest.find("{%").into_iter().chain(rest.find("{{")).min();

            match next_tag {
                Some(0) => {
                    // Starts with tag
                    if rest.starts_with("{%") {
                        self.advance(2);
                        self.in_tag = true;
                        Some(Token::BlockStart)
                    } else {
                        self.advance(2);
                        self.in_tag = true;
                        Some(Token::VarStart)
                    }
                }
                Some(idx) => {
                    // Text before tag
                    let text = rest[..idx].to_string();
                    self.advance(idx);
                    Some(Token::Text(text))
                }
                None => {
                    // All text
                    let text = rest.to_string();
                    self.advance(rest.len());
                    Some(Token::Text(text))
                }
            }
        } else {
            // In tag: skip whitespace
            let rest_trimmed = rest.trim_start();
            let skipped = rest.len() - rest_trimmed.len();
            self.advance(skipped);

            let rest = self.remaining();
            if rest.is_empty() {
                return None;
            }

            // Check tag ends
            if rest.starts_with("%}") {
                self.advance(2);
                self.in_tag = false;

                if self.trim_blocks {
                    let after = self.remaining();
                    if after.starts_with('\n') {
                        self.advance(1);
                    } else if after.starts_with("\r\n") {
                        self.advance(2);
                    }
                }

                return Some(Token::BlockEnd);
            }
            if rest.starts_with("}}") {
                self.advance(2);
                self.in_tag = false;
                return Some(Token::VarEnd);
            }

            // Symbols
            if rest.starts_with("==") {
                self.advance(2);
                return Some(Token::EqEq);
            }
            if rest.starts_with("+") {
                self.advance(1);
                return Some(Token::Plus);
            }
            if rest.starts_with(".") {
                self.advance(1);
                return Some(Token::Dot);
            }
            if rest.starts_with("[") {
                self.advance(1);
                return Some(Token::LBracket);
            }
            if rest.starts_with("]") {
                self.advance(1);
                return Some(Token::RBracket);
            }
            if rest.starts_with("(") {
                self.advance(1);
                return Some(Token::LParen);
            }
            if rest.starts_with(")") {
                self.advance(1);
                return Some(Token::RParen);
            }

            // Strings
            let first = rest.chars().next().unwrap();
            if first == '\'' || first == '"' {
                let quote = first;
                // find closing quote, handle escape
                let mut end_idx = 1;
                let mut s = String::new();
                let mut chars = rest[1..].chars();
                while let Some(c) = chars.next() {
                    if c == quote {
                        self.advance(end_idx + 1);
                        return Some(Token::StringLit(s));
                    }
                    if c == '\\' {
                        end_idx += 1;
                        if let Some(esc) = chars.next() {
                            end_idx += 1; // Count escape char length? usually 1 byte for n,t
                                          // Actually we need byte index.
                                          // `escape` char consumes bytes. `esc` is char.
                            end_idx += esc.len_utf8();

                            match esc {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                _ => s.push(esc),
                            }
                        }
                    } else {
                        end_idx += c.len_utf8();
                        s.push(c);
                    }
                }
                // Unterminated string
                return None;
            }

            // Identifiers / Keywords
            if first.is_alphabetic() || first == '_' {
                let _len = rest
                    .chars() // Renamed to _len to suppress warning
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .count();
                // Logic is safer with indices.
                let ident_str: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                self.advance(ident_str.len());

                return match ident_str.as_str() {
                    "if" => Some(Token::If),
                    "elif" => Some(Token::Elif),
                    "else" => Some(Token::Else),
                    "endif" => Some(Token::EndIf),
                    "for" => Some(Token::For),
                    "in" => Some(Token::In),
                    "endfor" => Some(Token::EndFor),
                    "and" => Some(Token::And),
                    "or" => Some(Token::Or),
                    "true" => Some(Token::True),
                    "false" => Some(Token::False),
                    _ => Some(Token::Ident(ident_str)),
                };
            }

            // Unknown char? Skip one.
            self.advance(1);
            self.next_token()
        }
    }
}
