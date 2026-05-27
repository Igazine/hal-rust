use crate::types::TokenData;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Identifier(String),
    Number(f64),
    String(String),
    Regex(String),
    
    Assign,    // =
    Question,  // ?
    Colon,     // :
    Rescue,    // ~
    At,        // @
    Hash,      // #
    Not,       // !
    Caret,     // ^
    Dot,       // .
    Comma,     // ,
    
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    
    Newline,
    EOF,
    Error(String),
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    line_start: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            line_start: 0,
        }
    }

    pub fn tokenize(&mut self) -> Vec<(Token, TokenData)> {
        let mut tokens = Vec::new();
        while self.pos < self.input.len() {
            let char = self.input[self.pos];

            if char.is_whitespace() {
                if char == '\n' {
                    tokens.push((Token::Newline, self.make_td()));
                    self.line += 1;
                    self.pos += 1;
                    self.line_start = self.pos;
                } else {
                    self.pos += 1;
                }
                continue;
            }

            if char == '/' && self.peek() == Some('/') {
                self.skip_comment();
                continue;
            }

            if char == '/' && self.is_regex_start(&tokens) {
                tokens.push((self.read_regex(), self.make_td()));
                continue;
            }

            if char == '-' && self.peek().map_or(false, |c| c.is_ascii_digit()) {
                tokens.push((self.read_number(), self.make_td()));
                continue;
            }

            if char.is_ascii_digit() {
                tokens.push((self.read_number(), self.make_td()));
                continue;
            }

            if char.is_alphabetic() || char == '_' {
                tokens.push((self.read_identifier(), self.make_td()));
                continue;
            }

            if char == '"' || char == '\'' {
                tokens.push((self.read_string(char), self.make_td()));
                continue;
            }

            let t = match char {
                '=' => Token::Assign,
                '?' => Token::Question,
                ':' => Token::Colon,
                '~' => Token::Rescue,
                '@' => Token::At,
                '#' => Token::Hash,
                '!' => Token::Not,
                '^' => Token::Caret,
                '.' => Token::Dot,
                ',' => Token::Comma,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                _ => Token::Error(format!("Unexpected character: {}", char)),
            };
            tokens.push((t, self.make_td()));
            self.pos += 1;
        }
        tokens.push((Token::EOF, self.make_td()));
        tokens
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    fn skip_comment(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos] != '\n' {
            self.pos += 1;
        }
    }

    fn is_regex_start(&self, tokens: &[(Token, TokenData)]) -> bool {
        if tokens.is_empty() { return true; }
        match &tokens.last().unwrap().0 {
            Token::Assign | Token::Question | Token::Colon | Token::Comma | 
            Token::LParen | Token::LBracket | Token::LBrace | Token::Newline | Token::Not => true,
            _ => false,
        }
    }

    fn read_regex(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1; // skip /
        while self.pos < self.input.len() && self.input[self.pos] != '/' {
            if self.input[self.pos] == '\\' { self.pos += 2; }
            else { self.pos += 1; }
        }
        if self.pos >= self.input.len() { return Token::Error("Unclosed regex literal".into()); }
        self.pos += 1; // skip /
        while self.pos < self.input.len() && self.input[self.pos].is_alphabetic() {
            self.pos += 1;
        }
        Token::Regex(self.input[start..self.pos].iter().collect())
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        if self.input[self.pos] == '-' { self.pos += 1; }
        while self.pos < self.input.len() && (self.input[self.pos].is_ascii_digit() || self.input[self.pos] == '.') {
            self.pos += 1;
        }
        let lit: String = self.input[start..self.pos].iter().collect();
        Token::Number(lit.parse().unwrap_or(0.0))
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.input.len() && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_') {
            self.pos += 1;
        }
        Token::Identifier(self.input[start..self.pos].iter().collect())
    }

    fn read_string(&mut self, quote: char) -> Token {
        self.pos += 1; // skip quote
        let mut val = String::new();
        while self.pos < self.input.len() && self.input[self.pos] != quote {
            if self.input[self.pos] == '\\' {
                self.pos += 1;
                if self.pos < self.input.len() {
                    match self.input[self.pos] {
                        'n' => val.push('\n'),
                        't' => val.push('\t'),
                        _ => val.push(self.input[self.pos]),
                    }
                }
            } else {
                val.push(self.input[self.pos]);
            }
            self.pos += 1;
        }
        if self.pos >= self.input.len() { return Token::Error("Unclosed string literal".into()); }
        self.pos += 1; // skip quote
        Token::String(val)
    }

    fn make_td(&self) -> TokenData {
        let mut end = self.pos;
        while end < self.input.len() && self.input[end] != '\n' {
            end += 1;
        }
        TokenData {
            line: self.line,
            line_text: self.input[self.line_start..end].iter().collect(),
        }
    }
}
