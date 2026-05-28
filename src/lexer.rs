#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text(String),
    BlockStart, // {%  or  {%-
    BlockEnd,   // %}  or  -%}
    VarStart,   // {{  or  {{-
    VarEnd,     // }}  or  -}}

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
    Not,
    True,
    False,
    Set,
    Is,

    // Symbols
    EqEq,     // ==
    Ne,       // !=
    Assign,   // =  (single, for {% set %})
    Plus,     // +
    Minus,    // -
    Percent,  // %
    Pipe,     // |
    Dot,      // .
    Colon,    // :
    Lt,       // <
    Gt,       // >
    Le,       // <=
    Ge,       // >=
    LBracket, // [
    RBracket, // ]
    LParen,   // (
    RParen,   // )
    Comma,    // ,

    // Data
    Ident(String),
    StringLit(String),
    IntLit(i64),
}

#[derive(Clone)]
pub struct Tokenizer<'a> {
    input: &'a str,
    cursor: usize,
    in_tag: bool,
    trim_blocks: bool,
    trim_next_start: bool, // set by -%} or -}} to strip whitespace from the next text
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            cursor: 0,
            in_tag: false,
            trim_blocks: true,
            trim_next_start: false,
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
            // Jinja2 comments {# ... #} — consume entirely, emit nothing.
            // Must be checked before the general {%/{{{ scan because {#
            // shares the `{` prefix but is neither a block nor a var tag.
            if rest.starts_with("{#") {
                // Find the closing #} — if absent, consume the rest (malformed template)
                let close = rest.find("#}").map(|i| i + 2).unwrap_or(rest.len());
                self.advance(close);
                // Respect trim_blocks: eat the newline that follows #} if present
                if self.trim_blocks {
                    let after = self.remaining();
                    if after.starts_with("\r\n") { self.advance(2); }
                    else if after.starts_with('\n') { self.advance(1); }
                }
                // A comment with a leading `-` ({#-) strips preceding whitespace from
                // the already-emitted text — we cannot retroactively trim a previous
                // token, but we can mark trim_next_start so the *following* text is
                // trimmed, which is the practical effect for generation-prompt blocks.
                if rest.starts_with("{#-") {
                    self.trim_next_start = true;
                }
                return self.next_token(); // skip: recurse to get the next real token
            }

            // Find first {{ or {%  (also matches {{- and {%-)
            let pos_block = rest.find("{%");
            let pos_var   = rest.find("{{");
            // Also skip past any {# that may appear before the next real tag
            let pos_comment = rest.find("{#");
            let next_tag  = match (pos_block, pos_var, pos_comment) {
                (Some(b), Some(v), Some(c)) => Some(b.min(v).min(c)),
                (Some(b), Some(v), None)    => Some(b.min(v)),
                (Some(b), None,    Some(c)) => Some(b.min(c)),
                (None,    Some(v), Some(c)) => Some(v.min(c)),
                (Some(b), None,    None)    => Some(b),
                (None,    Some(v), None)    => Some(v),
                (None,    None,    Some(c)) => Some(c),
                (None,    None,    None)    => None,
            };

            match next_tag {
                Some(0) => {
                    // We are sitting right at the tag opener — re-enter to handle {#
                    if rest.starts_with("{#") {
                        return self.next_token();
                    }
                    if rest.starts_with("{%-") {
                        self.advance(3);
                        self.in_tag = true;
                        Some(Token::BlockStart)
                    } else if rest.starts_with("{%") {
                        self.advance(2);
                        self.in_tag = true;
                        Some(Token::BlockStart)
                    } else if rest.starts_with("{{-") {
                        self.advance(3);
                        self.in_tag = true;
                        Some(Token::VarStart)
                    } else {
                        self.advance(2);
                        self.in_tag = true;
                        Some(Token::VarStart)
                    }
                }
                Some(idx) => {
                    // There is text before the tag
                    let raw_text = &rest[..idx];
                    let upcoming = &rest[idx..];

                    // {#- strips trailing whitespace from the preceding text too
                    let text = if upcoming.starts_with("{%-") || upcoming.starts_with("{{-") || upcoming.starts_with("{#-") {
                        raw_text.trim_end().to_string()
                    } else {
                        raw_text.to_string()
                    };

                    // -%} or -}} earlier set trim_next_start to strip leading whitespace
                    let text = if self.trim_next_start {
                        self.trim_next_start = false;
                        text.trim_start().to_string()
                    } else {
                        text
                    };

                    self.advance(idx);
                    if text.is_empty() {
                        // All whitespace consumed by trim — skip the empty token,
                        // and the next call will hit the {# or real tag at position 0
                        self.next_token()
                    } else {
                        Some(Token::Text(text))
                    }
                }
                None => {
                    // No more tags — rest is all text
                    let mut text = rest.to_string();
                    if self.trim_next_start {
                        self.trim_next_start = false;
                        text = text.trim_start().to_string();
                    }
                    self.advance(rest.len());
                    if text.is_empty() {
                        None
                    } else {
                        Some(Token::Text(text))
                    }
                }
            }
        } else {
            // In tag: skip leading whitespace
            let rest_trimmed = rest.trim_start();
            let skipped = rest.len() - rest_trimmed.len();
            self.advance(skipped);

            let rest = self.remaining();
            if rest.is_empty() {
                return None;
            }

            // Check tag ends — trim variants first
            if rest.starts_with("-%}") {
                self.advance(3);
                self.in_tag = false;
                self.trim_next_start = true; // strip all leading whitespace from next text
                return Some(Token::BlockEnd);
            }
            if rest.starts_with("%}") {
                self.advance(2);
                self.in_tag = false;
                if self.trim_blocks {
                    let after = self.remaining();
                    if after.starts_with("\r\n") {
                        self.advance(2);
                    } else if after.starts_with('\n') {
                        self.advance(1);
                    }
                }
                return Some(Token::BlockEnd);
            }
            if rest.starts_with("-}}") {
                self.advance(3);
                self.in_tag = false;
                self.trim_next_start = true;
                return Some(Token::VarEnd);
            }
            if rest.starts_with("}}") {
                self.advance(2);
                self.in_tag = false;
                return Some(Token::VarEnd);
            }

            // Multi-char symbols (check before single-char variants)
            if rest.starts_with("==") {
                self.advance(2);
                return Some(Token::EqEq);
            }
            if rest.starts_with("!=") {
                self.advance(2);
                return Some(Token::Ne);
            }
            if rest.starts_with('+') {
                self.advance(1);
                return Some(Token::Plus);
            }
            if rest.starts_with('-') {
                self.advance(1);
                return Some(Token::Minus);
            }
            if rest.starts_with('|') {
                self.advance(1);
                return Some(Token::Pipe);
            }
            if rest.starts_with('.') {
                self.advance(1);
                return Some(Token::Dot);
            }
            if rest.starts_with('[') {
                self.advance(1);
                return Some(Token::LBracket);
            }
            if rest.starts_with(']') {
                self.advance(1);
                return Some(Token::RBracket);
            }
            if rest.starts_with('(') {
                self.advance(1);
                return Some(Token::LParen);
            }
            if rest.starts_with(')') {
                self.advance(1);
                return Some(Token::RParen);
            }
            if rest.starts_with(',') {
                self.advance(1);
                return Some(Token::Comma);
            }
            if rest.starts_with('%') {
                self.advance(1);
                return Some(Token::Percent);
            }
            if rest.starts_with(':') {
                self.advance(1);
                return Some(Token::Colon);
            }
            // Comparison operators (check 2-char before 1-char)
            if rest.starts_with("<=") {
                self.advance(2);
                return Some(Token::Le);
            }
            if rest.starts_with(">=") {
                self.advance(2);
                return Some(Token::Ge);
            }
            if rest.starts_with('<') {
                self.advance(1);
                return Some(Token::Lt);
            }
            if rest.starts_with('>') {
                self.advance(1);
                return Some(Token::Gt);
            }
            // Single = (must come after == check)
            if rest.starts_with('=') {
                self.advance(1);
                return Some(Token::Assign);
            }

            let first = rest.chars().next().unwrap();

            // Integer literals
            if first.is_ascii_digit() {
                let int_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                self.advance(int_str.len());
                let n: i64 = int_str.parse().unwrap_or(0);
                return Some(Token::IntLit(n));
            }

            // String literals
            if first == '\'' || first == '"' {
                let quote = first;
                let mut end_idx = 1usize;
                let mut s = String::new();
                let mut chars = rest[1..].chars();
                loop {
                    match chars.next() {
                        None => return None, // unterminated string
                        Some(c) if c == quote => {
                            self.advance(end_idx + quote.len_utf8());
                            return Some(Token::StringLit(s));
                        }
                        Some('\\') => {
                            end_idx += 1;
                            match chars.next() {
                                None => return None,
                                Some(esc) => {
                                    end_idx += esc.len_utf8();
                                    match esc {
                                        'n'  => s.push('\n'),
                                        't'  => s.push('\t'),
                                        'r'  => s.push('\r'),
                                        '\'' => s.push('\''),
                                        '"'  => s.push('"'),
                                        '\\' => s.push('\\'),
                                        _    => s.push(esc),
                                    }
                                }
                            }
                        }
                        Some(c) => {
                            end_idx += c.len_utf8();
                            s.push(c);
                        }
                    }
                }
            }

            // Identifiers and keywords
            if first.is_alphabetic() || first == '_' {
                let ident_str: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                self.advance(ident_str.len());
                return match ident_str.as_str() {
                    "if"     => Some(Token::If),
                    "elif"   => Some(Token::Elif),
                    "else"   => Some(Token::Else),
                    "endif"  => Some(Token::EndIf),
                    "for"    => Some(Token::For),
                    "in"     => Some(Token::In),
                    "endfor" => Some(Token::EndFor),
                    "and"    => Some(Token::And),
                    "or"     => Some(Token::Or),
                    "not"    => Some(Token::Not),
                    "true"   => Some(Token::True),
                    "false"  => Some(Token::False),
                    "set"    => Some(Token::Set),
                    "is"     => Some(Token::Is),
                    _        => Some(Token::Ident(ident_str)),
                };
            }

            // Unknown character — skip and continue
            self.advance(1);
            self.next_token()
        }
    }
}
