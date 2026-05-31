use crate::types::{TokenData, HankError};
use crate::error_registry::HankErrorRegistry;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Identifier(String),
    Number(f64),
    String(String),
    
    Assign,    // =
    Question,  // ?
    Colon,     // :
    Rescue,    // ~
    At,        // @
    Hash,      // #
    Not,       // !
    Caret,     // ^
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
        let mut tokens = vec![];

        while self.pos < self.input.len() {
            let char = self.input[self.pos];

            if char.is_whitespace() {
                if char == '\n' {
                    let td = self.td();
                    self.pos += 1;
                    tokens.push((Token::Newline, td));
                    self.line += 1;
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

            let td = self.td();

            if char == '-' && self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                tokens.push((self.read_number(), td));
                continue;
            }

            if char.is_ascii_digit() {
                tokens.push((self.read_number(), td));
                continue;
            }

            if char.is_alphabetic() || char == '_' {
                tokens.push((self.read_identifier(), td));
                continue;
            }

            if char == '"' || char == '\'' {
                tokens.push((self.read_string(char), td));
                continue;
            }

            let token = match char {
                '=' => Token::Assign,
                '?' => Token::Question,
                ':' => Token::Colon,
                '~' => Token::Rescue,
                '@' => Token::At,
                '#' => Token::Hash,
                '!' => Token::Not,
                '^' => Token::Caret,
                ',' => Token::Comma,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                '.' => {
                    self.pos += 1;
                    Token::Error(HankErrorRegistry::create(HankError::UnexpectedCharacter, vec![".".to_string()], None, Some(self.line), Some(self.pos - self.line_start + 1), Some(&self.get_current_line_text())).message)
                }
                _ => {
                    self.pos += 1;
                    Token::Error(HankErrorRegistry::create(HankError::UnexpectedCharacter, vec![char.to_string()], None, None, None, None).message)
                }
            };

            if token != Token::Error("".to_string()) { // Dummy check to avoid double-increment for '.' and '_' match arms
                if char != '.' { // Dot and _ already incremented
                     self.pos += 1;
                }
                tokens.push((token, td));
            } else {
                 tokens.push((token, td));
            }
        }

        tokens.push((Token::EOF, self.td()));
        tokens
    }

    fn skip_comment(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos] != '\n' {
            self.pos += 1;
        }
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        let mut has_dot = false;
        
        if self.input[self.pos] == '-' {
            self.pos += 1;
        }

        while self.pos < self.input.len() {
            let c = self.input[self.pos];
            if c == '.' {
                if has_dot {
                    break;
                }
                has_dot = true;
                self.pos += 1;
            } else if c.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }

        // Roll back trailing dots
        while self.pos > start && self.input[self.pos - 1] == '.' {
            self.pos -= 1;
        }

        let s: String = self.input[start..self.pos].iter().collect();
        let val = s.parse::<f64>().unwrap_or(0.0);

        // Check for illegal suffix
        if self.pos < self.input.len() {
            let c = self.input[self.pos];
            if c.is_ascii_alphabetic() || c == '_' {
                while self.pos < self.input.len() && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_') {
                    self.pos += 1;
                }
                let full: String = self.input[start..self.pos].iter().collect();
                return Token::Error(HankErrorRegistry::create(HankError::UnexpectedCharacter, vec![full], None, None, None, None).message);
            }
        }

        Token::Number(val)
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.input.len() && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_') {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Token::Identifier(s)
    }

    fn read_string(&mut self, quote: char) -> Token {
        self.pos += 1; // skip quote
        let mut val = String::new();
        while self.pos < self.input.len() && self.input[self.pos] != quote {
            if self.input[self.pos] == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() { break; }
                match self.input[self.pos] {
                    'n' => val.push('\n'),
                    't' => val.push('\t'),
                    c => val.push(c),
                }
            } else {
                val.push(self.input[self.pos]);
            }
            self.pos += 1;
        }
        if self.pos >= self.input.len() {
            return Token::Error(HankErrorRegistry::create(HankError::UnclosedStringLiteral, vec![], None, None, None, None).message);
        }
        self.pos += 1; // skip quote
        Token::String(val)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos + 1).cloned()
    }

    fn td(&self) -> TokenData {
        TokenData {
            line: self.line,
            column: self.pos - self.line_start + 1,
            line_text: self.get_current_line_text(),
        }
    }

    fn get_current_line_text(&self) -> String {
        let mut end = self.pos;
        while end < self.input.len() && self.input[end] != '\n' {
            end += 1;
        }
        self.input[self.line_start..end].iter().collect()
    }
}
