use std::collections::HashMap;
use std::sync::Arc;
use std::cell::RefCell;
use regex_lite::Regex;

#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Number(f64),
    String(String),
    Array(Arc<RefCell<Vec<Value>>>),
    Object(Arc<RefCell<HashMap<String, Value>>>),
    Regex(Arc<RegexValue>),
    Task(Arc<TaskValue>),
}

#[derive(Debug)]
pub struct RegexValue {
    pub pattern: String,
    pub flags: String,
    pub engine: Option<Regex>,
}

#[derive(Debug)]
pub enum TaskValue {
    Native {
        name: String,
        func: NativeFunc,
    },
    User {
        params: Vec<Param>,
        body: Box<Expr>,
        closure: Arc<dyn Scope>,
    },
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub is_optional: bool,
    pub default_value: Option<Box<Expr>>,
}

pub type NativeFunc = fn(args: Vec<Value>, ctx: &dyn ExecutionContext) -> Value;

pub trait ExecutionContext {
    fn parse(&self, source: &str) -> Result<Box<Expr>, String>;
    fn eval(&self, node: &Expr) -> Value;
    fn call(&self, task: &Value, args: Vec<Value>) -> Value;
    fn scope(&self) -> &dyn Scope;
}

pub trait Scope: std::fmt::Debug {
    fn get(&self, name: &str) -> Value;
    fn set(&self, name: &str, val: Value);
    fn exists(&self, name: &str) -> bool;
}

pub trait IHALSerializable {
    fn serialize_hal(&self) -> String;
}

// AST Nodes
#[derive(Clone, Debug)]
pub enum Expr {
    Block(Vec<Expr>, TokenData),
    Assign(String, Box<Expr>, TokenData),
    Literal(Value, TokenData),
    Ident(String, bool, TokenData),
    Field(Box<Expr>, String, TokenData),
    FuncDef(Vec<Param>, Box<Expr>, TokenData),
    FuncCall(Box<Expr>, Vec<Expr>, TokenData),
    UnOp(String, Box<Expr>, TokenData),
    Object(HashMap<String, Expr>, TokenData),
    Array(Vec<Expr>, TokenData),
    FlowControl {
        condition: Box<Expr>,
        success: Box<Expr>,
        fallback: Option<Box<Expr>>,
        rescue: Option<Box<Expr>>,
        catch_var: Option<String>,
        token: TokenData,
    },
}

#[derive(Clone, Debug, Default)]
pub struct TokenData {
    pub line: usize,
    pub line_text: String,
}
