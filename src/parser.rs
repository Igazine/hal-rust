use crate::types::{Expr, Param, TokenData, Value, RegexValue};
use crate::lexer::Token;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Parser {
    tokens: Vec<(Token, TokenData)>,
    pos: usize,
    filename: String,
    macro_map: HashMap<String, String>,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, TokenData)>, filename: String, macro_map: HashMap<String, String>) -> Self {
        Self { tokens, pos: 0, filename, macro_map }
    }

    pub fn parse(&mut self) -> Result<Expr, String> {
        let td_root = self.peek_td();
        let mut stmts = Vec::new();
        
        // { MacroInclude }
        while !self.is_eof() {
            self.skip_newlines();
            if self.peek() != Token::At { break; }
            stmts.push(self.parse_include()?);
        }
        
        self.skip_newlines();
        if self.is_eof() {
            return Err(self.error("Script must conclude with a FuncDef or bare Block"));
        }

        // ( FuncDef | Block )
        let td_task = self.peek_td();
        let task = match self.peek() {
            Token::LParen => self.parse_func_def()?,
            Token::LBrace => {
                let body = Box::new(self.parse_block()?);
                Expr::FuncDef(Vec::new(), body, td_task)
            },
            _ => return Err(self.error("Script must conclude with a FuncDef or bare Block")),
        };
        
        stmts.push(task);
        
        self.skip_newlines();
        if !self.is_eof() {
            return Err(self.error("Unexpected content after script task definition"));
        }

        Ok(Expr::Block(stmts, td_root))
    }

    fn parse_statement(&mut self) -> Result<Expr, String> {
        self.skip_newlines();
        match &self.peek() {
            Token::Question => self.parse_flow_control(),
            Token::Caret => self.parse_return(),
            Token::At => self.parse_include(),
            _ => self.parse_expression(),
        }
    }

    fn parse_flow_control(&mut self) -> Result<Expr, String> {
        let token = self.consume(Token::Question)?;
        self.consume(Token::LParen)?;
        let condition = self.parse_expression()?;
        self.consume(Token::RParen)?;
        
        let success = Box::new(self.parse_block()?);
        
        let mut fallback = None;
        let mut rescue = None;
        let mut catch_var = None;
        
        let saved_pos = self.pos;
        self.skip_newlines();
        if self.peek() == Token::Colon {
            self.consume(Token::Colon)?;
            fallback = Some(Box::new(self.parse_block()?));
            self.skip_newlines();
        } else {
            self.pos = saved_pos;
        }
        
        let saved_pos = self.pos;
        self.skip_newlines();
        if self.peek() == Token::Rescue {
            self.consume(Token::Rescue)?;
            self.consume(Token::LParen)?;
            catch_var = Some(self.consume_identifier()?);
            self.consume(Token::RParen)?;
            rescue = Some(Box::new(self.parse_block()?));
        } else {
            self.pos = saved_pos;
        }
        
        Ok(Expr::FlowControl {
            condition: Box::new(condition),
            success,
            fallback,
            rescue,
            catch_var,
            token,
        })
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let (token, td) = self.peek_td_full();
        let expr = match token {
            Token::LParen => {
                if self.is_func_def_start() {
                    self.parse_func_def()?
                } else {
                    self.pos += 1;
                    let e = self.parse_expression()?;
                    self.consume(Token::RParen)?;
                    e
                }
            },
            Token::LBrace => self.parse_object_literal()?,
            Token::LBracket => self.parse_array_literal()?,
            Token::Not => {
                self.pos += 1;
                Expr::UnOp("!".into(), Box::new(self.parse_primary()?), td)
            },
            Token::Question => {
                self.pos += 1;
                Expr::UnOp("?".into(), Box::new(self.parse_primary()?), td)
            },
            Token::Hash => {
                self.pos += 1;
                let name = self.consume_identifier()?;
                Expr::Ident(name, true, td)
            },
            Token::Identifier(id) => {
                let name = id.clone();
                self.pos += 1;
                if self.peek() == Token::Assign {
                    self.consume(Token::Assign)?;
                    Expr::Assign(name, Box::new(self.parse_expression()?), td)
                } else {
                    Expr::Ident(name, false, td)
                }
            },
            Token::String(s) => {
                let val = s.clone();
                self.pos += 1;
                Expr::Literal(Value::String(val), td)
            },
            Token::Number(n) => {
                let val = n;
                self.pos += 1;
                Expr::Literal(Value::Number(val), td)
            },
            Token::Regex(r) => {
                let val = r.clone();
                self.pos += 1;
                Expr::Literal(Value::Regex(Arc::new(self.parse_regex_value(&val))), td)
            },
            _ => return Err(self.error(&format!("Unexpected token: {:?}", token))),
        };
        
        self.finish_primary(expr)
    }

    fn finish_primary(&mut self, mut expr: Expr) -> Result<Expr, String> {
        loop {
            match self.peek() {
                Token::Dot => {
                    self.consume(Token::Dot)?;
                    let field = self.consume_identifier()?;
                    expr = Expr::Field(Box::new(expr), field, self.peek_td());
                },
                Token::LParen => {
                    let args = self.parse_arg_list()?;
                    expr = Expr::FuncCall(Box::new(expr), args, self.peek_td());
                },
                _ => break,
            }
        }
        Ok(expr)
    }

    fn is_func_def_start(&self) -> bool {
        let mut p = self.pos + 1;
        let mut depth = 1;
        while p < self.tokens.len() && depth > 0 {
            match &self.tokens[p].0 {
                Token::LParen => depth += 1,
                Token::RParen => depth -= 1,
                _ => {}
            }
            p += 1;
        }
        // skip newlines
        while p < self.tokens.len() {
            if let Token::Newline = &self.tokens[p].0 { p += 1; }
            else { break; }
        }
        p < self.tokens.len() && matches!(&self.tokens[p].0, Token::LBrace)
    }

    fn parse_func_def(&mut self) -> Result<Expr, String> {
        let td = self.peek_td();
        self.consume(Token::LParen)?;
        let mut params = Vec::new();
        if self.peek() != Token::RParen {
            params.push(self.parse_param()?);
            while self.peek() == Token::Comma {
                self.consume(Token::Comma)?;
                params.push(self.parse_param()?);
            }
        }
        self.consume(Token::RParen)?;
        let body = Box::new(self.parse_block()?);
        Ok(Expr::FuncDef(params, body, td))
    }

    fn parse_param(&mut self) -> Result<Param, String> {
        let mut is_optional = false;
        if self.peek() == Token::Question {
            self.consume(Token::Question)?;
            is_optional = true;
        }
        let name = self.consume_identifier()?;
        let mut default_value = None;
        if self.peek() == Token::Assign {
            self.consume(Token::Assign)?;
            default_value = Some(Box::new(self.parse_expression()?));
            is_optional = true;
        }
        Ok(Param { name, is_optional, default_value })
    }

    fn parse_block(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBrace)?;
        let mut stmts = Vec::new();
        while self.peek() != Token::RBrace && !self.is_eof() {
            self.skip_newlines();
            if self.peek() == Token::RBrace { break; }
            stmts.push(self.parse_statement()?);
        }
        self.consume(Token::RBrace)?;
        Ok(Expr::Block(stmts, td))
    }

    fn parse_object_literal(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBrace)?;
        let mut fields = HashMap::new();
        while self.peek() != Token::RBrace && !self.is_eof() {
            self.skip_newlines();
            if self.peek() == Token::RBrace { break; }
            let key = self.consume_identifier()?;
            self.consume(Token::Colon)?;
            let val = self.parse_expression()?;
            fields.insert(key, val);
            if self.peek() == Token::Comma { self.consume(Token::Comma)?; }
        }
        self.consume(Token::RBrace)?;
        Ok(Expr::Object(fields, td))
    }

    fn parse_array_literal(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBracket)?;
        let mut items = Vec::new();
        while self.peek() != Token::RBracket && !self.is_eof() {
            self.skip_newlines();
            if self.peek() == Token::RBracket { break; }
            items.push(self.parse_expression()?);
            if self.peek() == Token::Comma { self.consume(Token::Comma)?; }
        }
        self.consume(Token::RBracket)?;
        Ok(Expr::Array(items, td))
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, String> {
        self.consume(Token::LParen)?;
        let mut args = Vec::new();
        self.skip_newlines();
        if self.peek() != Token::RParen {
            args.push(self.parse_expression()?);
            loop {
                self.skip_newlines();
                if self.peek() == Token::Comma {
                    self.consume(Token::Comma)?;
                    self.skip_newlines();
                    args.push(self.parse_expression()?);
                } else { break; }
            }
        }
        self.skip_newlines();
        self.consume(Token::RParen)?;
        Ok(args)
    }

    fn parse_return(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::Caret)?;
        let mut val = None;
        if !self.is_eof() && !matches!(self.peek(), Token::Newline | Token::RBrace | Token::RBracket | Token::Comma) {
            val = Some(Box::new(self.parse_expression()?));
        }
        Ok(Expr::UnOp("^".into(), val.unwrap_or(Box::new(Expr::Literal(Value::Void, td.clone()))), td))
    }

    fn parse_include(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::At)?;
        let raw_path = match self.peek() {
            Token::String(s) => {
                let val = s.clone();
                self.pos += 1;
                val
            },
            Token::Identifier(id) => {
                let val = id.clone();
                self.pos += 1;
                val
            },
            _ => return Err(self.error("Expected path string or identifier after @")),
        };
        
        let content = self.macro_map.get(&raw_path)
            .ok_or_else(|| self.error(&format!("Macro resource not found: @{}", raw_path)))?;
            
        let task_name = std::path::Path::new(&raw_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&raw_path)
            .to_string();
            
        let mut lexer = crate::lexer::Lexer::new(content);
        let tokens = lexer.tokenize();
        let mut sub_parser = Parser::new(tokens, raw_path.clone(), self.macro_map.clone());
        
        // Everything is a TaskDef now
        let task_ast = sub_parser.parse()?;
        
        Ok(Expr::Assign(task_name, Box::new(task_ast), td))
    }

    fn parse_regex_value(&self, lit: &str) -> RegexValue {
        let parts: Vec<&str> = lit.split('/').collect();
        if parts.len() >= 3 {
            RegexValue {
                pattern: parts[1].to_string(),
                flags: parts[2].to_string(),
                engine: None,
            }
        } else {
            RegexValue { pattern: lit.to_string(), flags: "".into(), engine: None }
        }
    }

    fn consume_identifier(&mut self) -> Result<String, String> {
        match self.peek() {
            Token::Identifier(id) => {
                let name = id.clone();
                self.pos += 1;
                Ok(name)
            },
            _ => Err(self.error(&format!("Expected identifier, found {:?}", self.peek()))),
        }
    }

    fn consume(&mut self, t: Token) -> Result<TokenData, String> {
        let (token, td) = self.peek_td_full();
        if std::mem::discriminant(&token) == std::mem::discriminant(&t) {
            self.pos += 1;
            Ok(td)
        } else {
            Err(self.error(&format!("Expected {:?}, found {:?}", t, token)))
        }
    }

    fn peek(&self) -> Token {
        self.tokens.get(self.pos).map(|(t, _)| t.clone()).unwrap_or(Token::At) // Dummy default
    }

    fn peek_td(&self) -> TokenData {
        self.tokens.get(self.pos).map(|(_, td)| td.clone()).unwrap_or_default()
    }

    fn peek_td_full(&self) -> (Token, TokenData) {
        self.tokens.get(self.pos).cloned().unwrap_or((Token::At, TokenData::default()))
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.pos += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.peek(), Token::EOF)
    }

    fn error(&self, msg: &str) -> String {
        let td = self.peek_td();
        format!("ERROR: {} in {} at\n\t{}:\t{}", msg, self.filename, td.line, td.line_text)
    }
}
