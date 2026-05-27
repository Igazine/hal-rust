use hal::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use std::cell::RefCell;
use std::env;

struct Runner {
    path_cache: HashMap<String, String>,
    ast_cache: HashMap<String, Expr>,
    macro_map: HashMap<String, String>,
    core_scope: Arc<HALScope>,
}

impl Runner {
    fn new() -> Self {
        let core = Arc::new(HALScope {
            values: RefCell::new(HashMap::new()),
            parent: None,
        });
        Self {
            path_cache: HashMap::new(),
            ast_cache: HashMap::new(),
            macro_map: HashMap::new(),
            core_scope: core,
        }
    }

    fn register_std(&self) {
        self.core_scope.set("log", Value::Object(Arc::new(RefCell::new({
            let mut log_mod = HashMap::new();
            log_mod.insert("print".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "log.print".into(),
                func: |args, _| {
                    let strs: Vec<String> = args.iter().map(|a| val_to_string(a)).collect();
                    println!("{}", strs.join(" "));
                    Value::Void
                }
            })));
            log_mod
        }))));

        self.core_scope.set("runtime", Value::Object(Arc::new(RefCell::new({
            let mut runtime_mod = HashMap::new();
            runtime_mod.insert("halt".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "runtime.halt".into(),
                func: |args, _| {
                    let code = if let Some(Value::Number(n)) = args.get(0) { *n as i32 } else { 0 };
                    std::process::exit(code);
                }
            })));
            runtime_mod.insert("elapsedTime".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "runtime.elapsedTime".into(),
                func: |_, _| { Value::Number(0.0) }
            })));
            runtime_mod
        }))));

        self.core_scope.set("env", Value::Object(Arc::new(RefCell::new({
            let mut env_mod = HashMap::new();
            env_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "env.get".into(),
                func: |args, _| {
                    if let Some(arg0) = args.get(0) { let _key = val_to_string(arg0); }
                    Value::Void
                }
            })));
            env_mod.insert("set".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "env.set".into(),
                func: |_, _| { Value::Void }
            })));
            env_mod.insert("keys".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "env.keys".into(),
                func: |_, _| { Value::Array(Arc::new(RefCell::new(vec![]))) }
            })));
            env_mod
        }))));

        self.core_scope.set("str", Value::Object(Arc::new(RefCell::new({
            let mut str_mod = HashMap::new();
            str_mod.insert("length".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "str.length".into(),
                func: |args, _| {
                    if let Some(Value::String(s)) = args.get(0) { return Value::Number(s.len() as f64); }
                    Value::Void
                }
            })));
            str_mod.insert("format".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "str.format".into(),
                func: |args, _| {
                    if args.is_empty() { return Value::Void; }
                    let mut res = val_to_string(&args[0]);
                    for i in 1..args.len() {
                        res = res.replace(&format!("%{}", i), &val_to_string(&args[i]));
                    }
                    Value::String(res)
                }
            })));
            str_mod.insert("concat".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "str.concat".into(),
                func: |args, _| {
                    let res: String = args.iter().map(|a| val_to_string(a)).collect();
                    Value::String(res)
                }
            })));
            str_mod.insert("trim".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "str.trim".into(),
                func: |args, _| {
                    if let Some(Value::String(s)) = args.get(0) { return Value::String(s.trim().to_string()); }
                    Value::Void
                }
            })));
            str_mod
        }))));

        self.core_scope.set("regex", Value::Object(Arc::new(RefCell::new({
            let mut regex_mod = HashMap::new();
            regex_mod.insert("parse".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "regex.parse".into(),
                func: |args, _| {
                    if args.is_empty() { return Value::Void; }
                    let pattern = val_to_string(&args[0]);
                    let flags = if args.len() > 1 { val_to_string(&args[1]) } else { "".into() };
                    let mut final_flags = String::new();
                    if flags.contains('i') { final_flags.push('i'); }
                    if flags.contains('m') { final_flags.push('m'); }
                    let mut final_pattern = pattern.clone();
                    if !final_flags.is_empty() { final_pattern = format!("(?{}){}", final_flags, pattern); }
                    let re = regex_lite::Regex::new(&final_pattern).ok();
                    Value::Regex(Arc::new(RegexValue { pattern, flags, engine: re }))
                }
            })));
            regex_mod.insert("match".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "regex.match".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    let s = val_to_string(&args[0]);
                    match &args[1] {
                        Value::Regex(rv) => {
                            if let Some(re) = &rv.engine { if re.is_match(&s) { return Value::Number(1.0); } }
                            Value::Void
                        },
                        other => if s.contains(&val_to_string(other)) { Value::Number(1.0) } else { Value::Void }
                    }
                }
            })));
            regex_mod
        }))));

        self.core_scope.set("math", Value::Object(Arc::new(RefCell::new({
            let mut math_mod = HashMap::new();
            math_mod.insert("add".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.add".into(),
                func: |args, _| {
                    let sum: f64 = args.iter().map(|a| match a { Value::Number(n) => *n, _ => 0.0 }).sum();
                    Value::Number(sum)
                }
            })));
            math_mod.insert("sub".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.sub".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let (Value::Number(a), Value::Number(b)) = (&args[0], &args[1]) { return Value::Number(a - b); }
                    Value::Void
                }
            })));
            math_mod.insert("mul".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.mul".into(),
                func: |args, _| {
                    if args.is_empty() { return Value::Number(0.0); }
                    let res: f64 = args.iter().map(|a| match a { Value::Number(n) => *n, _ => 1.0 }).product();
                    Value::Number(res)
                }
            })));
            math_mod.insert("gt".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.gt".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let (Value::Number(a), Value::Number(b)) = (&args[0], &args[1]) { if a > b { return Value::Number(1.0); } }
                    Value::Void
                }
            })));
            math_mod.insert("lt".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.lt".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let (Value::Number(a), Value::Number(b)) = (&args[0], &args[1]) { if a < b { return Value::Number(1.0); } }
                    Value::Void
                }
            })));
            math_mod.insert("eq".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "math.eq".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if val_to_string(&args[0]) == val_to_string(&args[1]) { return Value::Number(1.0); }
                    Value::Void
                }
            })));
            math_mod
        }))));

        self.core_scope.set("logic", Value::Object(Arc::new(RefCell::new({
            let mut logic_mod = HashMap::new();
            logic_mod.insert("and".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "logic.and".into(),
                func: |args, _| {
                    if args.is_empty() { return Value::Void; }
                    let mut last = Value::Void;
                    for a in args {
                        if matches!(a, Value::Void) { return Value::Void; }
                        last = a.clone();
                    }
                    last
                }
            })));
            logic_mod.insert("or".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "logic.or".into(),
                func: |args, _| {
                    for a in args {
                        if !matches!(a, Value::Void) { return a.clone(); }
                    }
                    Value::Void
                }
            })));
            logic_mod
        }))));

        
        self.core_scope.set("arr", Value::Object(Arc::new(RefCell::new({
            let mut arr_mod = HashMap::new();
            arr_mod.insert("length".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "arr.length".into(),
                func: |args, _| {
                    if let Some(Value::Array(a)) = args.get(0) { return Value::Number(a.borrow().len() as f64); }
                    Value::Void
                }
            })));
            arr_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "arr.get".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let (Value::Array(a), Value::Number(n)) = (&args[0], &args[1]) {
                        let idx = *n as usize;
                        if let Some(val) = a.borrow().get(idx) { return val.clone(); }
                    }
                    Value::Void
                }
            })));
            arr_mod.insert("push".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "arr.push".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let Value::Array(a) = &args[0] {
                        a.borrow_mut().push(args[1].clone());
                    }
                    Value::Void
                }
            })));
            arr_mod.insert("pop".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "arr.pop".into(),
                func: |args, _| {
                    if let Some(Value::Array(a)) = args.get(0) {
                        return a.borrow_mut().pop().unwrap_or(Value::Void);
                    }
                    Value::Void
                }
            })));
            arr_mod.insert("each".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "arr.each".into(),
                func: |args, ctx| {
                    if args.len() < 2 { return Value::Void; }
                    if let (Value::Array(a), Value::Task(t)) = (&args[0], &args[1]) {
                        // SHALLOW SNAPSHOT
                        let items = a.borrow().clone();
                        for (idx, item) in items.iter().enumerate() {
                            ctx.call(&Value::Task(t.clone()), vec![item.clone(), Value::Number(idx as f64)]);
                        }
                    }
                    Value::Void
                }
            })));
            arr_mod
        }))));

        self.core_scope.set("obj", Value::Object(Arc::new(RefCell::new({
            let mut obj_mod = HashMap::new();
            obj_mod.insert("get".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "obj.get".into(),
                func: |args, _| {
                    if args.len() < 2 { return Value::Void; }
                    if let Value::Object(map) = &args[0] {
                        let key = val_to_string(&args[1]);
                        if let Some(val) = map.borrow().get(&key) { return val.clone(); }
                    }
                    Value::Void
                }
            })));
            obj_mod.insert("keys".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "obj.keys".into(),
                func: |args, _| {
                    if let Some(Value::Object(map)) = args.get(0) {
                        let keys: Vec<Value> = map.borrow().keys().map(|k| Value::String(k.clone())).collect();
                        return Value::Array(Arc::new(RefCell::new(keys)));
                    }
                    Value::Void
                }
            })));
            obj_mod.insert("values".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "obj.values".into(),
                func: |args, _| {
                    if let Some(Value::Object(map)) = args.get(0) {
                        let vals: Vec<Value> = map.borrow().values().cloned().collect();
                        return Value::Array(Arc::new(RefCell::new(vals)));
                    }
                    Value::Void
                }
            })));
            obj_mod
        }))));

        self.core_scope.set("json", Value::Object(Arc::new(RefCell::new({
            let mut json_mod = HashMap::new();
            json_mod.insert("parse".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "json.parse".into(),
                func: |args, _| {
                    if let Some(Value::String(s)) = args.get(0) {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(s) { return map_json_to_hal(data); }
                    }
                    Value::Void
                }
            })));
            json_mod.insert("stringify".into(), Value::Task(Arc::new(TaskValue::Native {
                name: "json.stringify".into(),
                func: |args, _| {
                    if let Some(v) = args.get(0) {
                        let j = map_hal_to_json(v);
                        if let Ok(s) = serde_json::to_string(&j) { return Value::String(s); }
                    }
                    Value::Void
                }
            })));
            json_mod
        }))));
    }

    fn load(&mut self, path: &str) -> Result<String, String> {
        let abs_path = fs::canonicalize(path).map_err(|e| e.to_string())?.to_string_lossy().to_string();
        if self.ast_cache.contains_key(&abs_path) { return Ok(abs_path); }

        self.preprocess(&abs_path, &mut Vec::new())?;
        let content = self.path_cache.get(&abs_path).cloned().ok_or_else(|| format!("File not loaded: {}", abs_path))?;
        let mut lexer = Lexer::new(&content);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, abs_path.clone(), self.macro_map.clone());
        let ast = parser.parse()?;
        
        self.ast_cache.insert(abs_path.clone(), ast);
        Ok(abs_path)
    }

    fn unload(&mut self, path: &str) {
        if let Ok(abs_path) = fs::canonicalize(path) {
            let abs_path_str = abs_path.to_string_lossy().to_string();
            self.ast_cache.remove(&abs_path_str);
            self.path_cache.remove(&abs_path_str);
        }
    }

    fn run(&mut self, path: &str, args: Vec<Value>) -> Result<Value, String> {
        let abs_path = self.load(path)?;
        let ast = self.ast_cache.get(&abs_path).unwrap();
        
        let mut interp = Interpreter::new(None, self.core_scope.clone() as Arc<dyn Scope>);
        let script_task = interp.run(ast);
        if let Value::Task(_) = script_task {
            match interp.call(&script_task, args, &interp.global_scope.clone()) {
                EvalResult::Value(v) | EvalResult::Return(v) => Ok(v),
                EvalResult::Error(e) => Err(e),
            }
        } else { Err("Script did not evaluate to a Task".into()) }
    }

    fn preprocess(&mut self, path: &str, stack: &mut Vec<String>) -> Result<(), String> {
        if stack.contains(&path.to_string()) { return Err(format!("Circular Dependency: {}", path)); }
        if self.path_cache.contains_key(path) { return Ok(()); }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.path_cache.insert(path.to_string(), content.clone());
        stack.push(path.to_string());
        let macros = self.scan_macros(&content);
        let parent_dir = Path::new(path).parent().unwrap();
        for m in macros {
            let m_path = self.resolve_path(&m, parent_dir);
            let m_abs = fs::canonicalize(&m_path).map_err(|e| format!("Failed to resolve @{}: {}", m, e))?.to_string_lossy().to_string();
            self.preprocess(&m_abs, stack)?;
            self.macro_map.insert(m.clone(), self.path_cache.get(&m_abs).unwrap().clone());
        }
        stack.pop();
        Ok(())
    }

    fn scan_macros(&self, content: &str) -> Vec<String> {
        let mut lexer = Lexer::new(content);
        let tokens = lexer.tokenize();
        let mut macros = Vec::new();
        for i in 0..tokens.len().saturating_sub(1) {
            if let Token::At = &tokens[i].0 {
                match &tokens[i+1].0 {
                    Token::String(s) | Token::Identifier(s) => macros.push(s.clone()),
                    _ => {}
                }
            }
        }
        macros
    }

    fn resolve_path(&self, m: &str, base: &Path) -> PathBuf {
        let p = Path::new(m);
        if p.is_absolute() { return p.to_path_buf(); }
        let joined = base.join(p);
        if joined.extension().is_none() { if joined.with_extension("hal").exists() { return joined.with_extension("hal"); } }
        joined
    }
}

fn map_json_to_hal(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Void,
        serde_json::Value::Bool(b) => if b { Value::Number(1.0) } else { Value::Void },
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => Value::Array(Arc::new(RefCell::new(a.into_iter().map(map_json_to_hal).collect()))),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, val) in o { map.insert(k, map_json_to_hal(val)); }
            Value::Object(Arc::new(RefCell::new(map)))
        }
    }
}

fn map_hal_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Void => serde_json::Value::Null,
        Value::Number(n) => serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap()),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(a) => serde_json::Value::Array(a.borrow().iter().map(map_hal_to_json).collect()),
        Value::Object(o) => {
            let mut map = serde_json::Map::new();
            for (k, val) in o.borrow().iter() { map.insert(k.clone(), map_hal_to_json(val)); }
            serde_json::Value::Object(map)
        },
        _ => serde_json::Value::Null,
    }
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Void => "null".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Object(_) => "{Object}".into(),
        Value::Regex(_) => "[Regex]".into(),
        Value::Task(_) => "[Task]".into(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { run_conformance(); return; }
    let mut runner = Runner::new();
    runner.register_std();
    let mut hal_args = Vec::new();
    if args.len() > 2 { for arg in &args[2..] { hal_args.push(Value::String(arg.clone())); } }
    match runner.run(&args[1], hal_args) {
        Ok(val) => { let code = match val { Value::Number(n) => n as i32, _ => 0 }; std::process::exit(code); },
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
    }
}

fn run_conformance() {
    let mut root = env::current_dir().unwrap();
    if !root.join("vendor/hal").exists() {
        if let Some(parent) = root.parent() {
            if parent.join("vendor/hal").exists() {
                root = parent.to_path_buf();
            } else if let Some(gparent) = parent.parent() {
                if gparent.join("vendor/hal").exists() {
                    root = gparent.to_path_buf();
                }
            }
        }
    }
    let workspace_root = root.join("vendor/hal");
    
    let tests = [
        "test/conformance/01_literals.hal",
        "test/conformance/02_gates.hal",
        "test/conformance/03_scoping.hal",
        "test/conformance/04_hoisting.hal",
        "test/conformance/05_params.hal",
        "test/conformance/06_macros.hal",
        "test/conformance/07_returns.hal",
        "test/conformance/08_host_args.hal",
        "test/conformance/09_deep_nesting.hal",
        "test/conformance/10_edge_cases.hal",
        "test/conformance/11_regex_parse.hal",
        "test/conformance/12_data_advanced.hal",
        "test/conformance/13_logic_module.hal",
    ];
    for t in tests {
        println!("--- Running: {} ---", t);
        let mut runner = Runner::new();
        runner.register_std();
        let path = workspace_root.join(t);
        let mut args = Vec::new();
        if t.ends_with("08_host_args.hal") { args.push(Value::String("Tamas".into())); }
        if let Err(e) = runner.run(&path.to_string_lossy(), args) { println!("Test Failed: {}", e); }
        println!("--------------------\n");
    }
}
