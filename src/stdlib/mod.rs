use crate::types::{Value, TaskValue, Scope, OpaqueValue, Arc, EvalResult, HankError, HankErrorValue, NativeFunc, HankExtension, ExecutionContext, Expr, ValueType, ErrorValue};
use crate::error_registry::HankErrorRegistry;
use std::collections::HashMap;
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn wasm_log(s: &str);
}

fn hank_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Void, Value::Void) => true,
        (Value::Number(n1), Value::Number(n2)) => n1 == n2,
        (Value::String(s1), Value::String(s2)) => s1 == s2,
        (Value::Array(a1), Value::Array(a2)) => {
            let a1 = a1.borrow();
            let a2 = a2.borrow();
            if a1.len() != a2.len() { return false; }
            for i in 0..a1.len() {
                if !hank_equals(&a1[i], &a2[i]) { return false; }
            }
            true
        },
        (Value::Map(o1), Value::Map(o2)) => {
            let o1 = o1.borrow();
            let o2 = o2.borrow();
            if o1.len() != o2.len() { return false; }
            for (k, v1) in o1.iter() {
                if let Some(v2) = o2.get(k) {
                    if !hank_equals(v1, v2) { return false; }
                } else {
                    return false;
                }
            }
            true
        },
        (Value::Opaque(ov1), Value::Opaque(ov2)) => {
            ov1.label == ov2.label && Arc::ptr_eq(ov1, ov2)
        },
        (Value::Error(e1), Value::Error(e2)) => {
            if e1.code != e2.code || e1.args.len() != e2.args.len() { return false; }
            for i in 0..e1.args.len() {
                if !hank_equals(&e1.args[i], &e2.args[i]) { return false; }
            }
            true
        },
        _ => false,
    }
}

pub struct StdLib {
    pub env_state: Arc<RefCell<HashMap<String, Value>>>,
}

impl StdLib {
    pub fn new() -> Self {
        Self {
            env_state: Arc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl HankExtension for StdLib {
    fn name(&self) -> &str { "StdLib" }
    fn get_tasks(&self) -> HashMap<String, NativeFunc> {
        get_stdlib_tasks(self.env_state.clone())
    }
}

fn wrap_native<F>(f: F) -> NativeFunc 
where F: for<'a> Fn(Vec<Value>, &'a dyn ExecutionContext) -> EvalResult + 'static 
{
    Arc::new(f)
}

pub fn get_stdlib_tasks(env_state: Arc<RefCell<HashMap<String, Value>>>) -> HashMap<String, NativeFunc> {
    let mut tasks = HashMap::new();

    // --- log ---
    tasks.insert("log_print".into(), wrap_native(|args: Vec<Value>, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&msg);
            #[cfg(not(target_arch = "wasm32"))]
            println!("{}", msg);
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("log_error".into(), wrap_native(|args: Vec<Value>, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[ERROR] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", msg);
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("log_warn".into(), wrap_native(|args: Vec<Value>, _| {
            let msg = args.iter().map(|a| val_to_string(a)).collect::<Vec<_>>().join(" ");
            #[cfg(target_arch = "wasm32")]
            wasm_log(&format!("[WARN] {}", msg));
            #[cfg(not(target_arch = "wasm32"))]
            println!("[WARN] {}", msg);
            EvalResult::Value(Value::Void)
        }));

    // --- runtime ---
    tasks.insert("runtime_halt".into(), wrap_native(|args: Vec<Value>, _| {
            let code = if let Some(Value::Number(n)) = args.get(0) { *n as i32 } else { 0 };
            std::process::exit(code);
        }));
    tasks.insert("runtime_elapsedTime".into(), wrap_native(|_: Vec<Value>, _| EvalResult::Value(Value::Number(0.0))));
    tasks.insert("runtime_signal".into(), wrap_native(|args: Vec<Value>, _| {
            if !args.is_empty() {
                println!("[SIGNAL] {}", val_to_string(&args[0]));
            }
            EvalResult::Value(Value::Void)
        }));

    // --- loop ---
    tasks.insert("loop_while".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let cond = &args[0];
            let body = &args[1];
            let mut last = Value::Void;
            loop {
                let cond_val = ctx.call(cond, vec![]);
                if ctx.is_error(&cond_val) { return EvalResult::Error(cond_val); }
                if matches!(cond_val, Value::Void) { break; }
                
                let res = ctx.call(body, vec![]);
                if let Value::Opaque(op) = &res {
                    if op.label == "__ControlFlow" && op.data.downcast_ref::<String>().map(|s| s == "Break").unwrap_or(false) {
                        break;
                    }
                }
                if ctx.is_error(&res) { return EvalResult::Error(res); }
                last = res;
            }
            EvalResult::Value(last)
        }));
    tasks.insert("loop_break".into(), wrap_native(|_: Vec<Value>, _| {
            EvalResult::Value(Value::Opaque(Arc::new(OpaqueValue { label: "__ControlFlow".into(), data: Box::new("Break".to_string()) })))
        }));

    // --- env ---
    let env_get_state = env_state.clone();
    tasks.insert("env_get".into(), wrap_native(move |args: Vec<Value>, _| {
            if let Some(key) = args.get(0) {
                let key_str = val_to_string(key);
                let state = env_get_state.borrow();
                return EvalResult::Value(state.get(&key_str).cloned().unwrap_or(Value::Void));
            }
            EvalResult::Value(Value::Void)
        }));
    let env_set_state = env_state.clone();
    tasks.insert("env_set".into(), wrap_native(move |args: Vec<Value>, _| {
            if let (Some(key), Some(val)) = (args.get(0), args.get(1)) {
                let key_str = val_to_string(key);
                env_set_state.borrow_mut().insert(key_str, val.clone());
            }
            EvalResult::Value(Value::Void)
        }));
    let env_keys_state = env_state.clone();
    tasks.insert("env_keys".into(), wrap_native(move |_: Vec<Value>, _| {
            let state = env_keys_state.borrow();
            let keys: Vec<Value> = state.keys().map(|k| Value::String(k.clone())).collect();
            EvalResult::Value(Value::Array(Arc::new(RefCell::new(keys))))
        }));

    // --- math ---
    tasks.insert("math_add".into(), wrap_native(|args: Vec<Value>, _| {
            let mut sum = 0.0;
            for a in args {
                if let Value::Number(n) = a { sum += n; }
                else { return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", a.get_type())), Value::String("math_add".into())] }))); }
            }
            EvalResult::Value(Value::Number(sum))
        }));
    tasks.insert("math_sub".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(Value::Number(a - b)),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math_sub".into())] })))
                }
            }
        }));
    tasks.insert("math_mul".into(), wrap_native(|args: Vec<Value>, _| {
            let mut res = 1.0;
            if args.is_empty() { return EvalResult::Value(Value::Number(0.0)); }
            for a in args {
                if let Value::Number(n) = a { res *= n; }
                else { return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", a.get_type())), Value::String("math_mul".into())] }))); }
            }
            EvalResult::Value(Value::Number(res))
        }));
    tasks.insert("math_div".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b != 0.0 { EvalResult::Value(Value::Number(a / b)) } else { EvalResult::Value(Value::Void) }
                },
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math_div".into())] })))
                }
            }
        }));
    tasks.insert("math_mod".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b != 0.0 { EvalResult::Value(Value::Number(a % b)) } else { EvalResult::Value(Value::Void) }
                },
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math_mod".into())] })))
                }
            }
        }));
    tasks.insert("math_gt".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(if a > b { Value::Number(1.0) } else { Value::Void }),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math_gt".into())] })))
                }
            }
        }));
    tasks.insert("math_lt".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            match (args.get(0).unwrap(), args.get(1).unwrap()) {
                (Value::Number(a), Value::Number(b)) => EvalResult::Value(if a < b { Value::Number(1.0) } else { Value::Void }),
                (a, b) => {
                    let faulty = if a.get_type() != ValueType::Number { a } else { b };
                    EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", faulty.get_type())), Value::String("math_lt".into())] })))
                }
            }
        }));
    tasks.insert("math_eq".into(), wrap_native(|args: Vec<Value>, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if hank_equals(a, b) { EvalResult::Value(Value::Number(1.0)) } else { EvalResult::Value(Value::Void) } } else { EvalResult::Value(Value::Void) } }));

    // --- str ---
    tasks.insert("str_length".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::String(s)) = args.get(0) { return EvalResult::Value(Value::Number(s.chars().count() as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("str_length".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("str_format".into(), wrap_native(|args: Vec<Value>, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let mut res = val_to_string(&args[0]);
            for i in 1..args.len() {
                res = res.replace(&format!("%{}", i), &val_to_string(&args[i]));
            }
            EvalResult::Value(Value::String(res))
        }));
    tasks.insert("str_concat".into(), wrap_native(|args: Vec<Value>, _| { EvalResult::Value(Value::String(args.iter().map(|a| val_to_string(a)).collect())) }));
    tasks.insert("str_trim".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::String(s)) = args.get(0) { return EvalResult::Value(Value::String(s.trim().to_string())); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("str_trim".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));

    // --- num ---
    tasks.insert("num_parse".into(), wrap_native(|args: Vec<Value>, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let s = val_to_string(&args[0]);
            let mut base = if let Some(Value::Number(n)) = args.get(1) { *n as u32 } else { 0 };

            let final_s = if base == 0 {
                if s.starts_with("0x") { base = 16; &s[2..] }
                else if s.starts_with("0b") { base = 2; &s[2..] }
                else if s.starts_with("0o") { base = 8; &s[2..] }
                else { base = 10; &s }
            } else { &s };

            if let Ok(n) = i64::from_str_radix(final_s, base) {
                EvalResult::Value(Value::Number(n as f64))
            } else if base == 10 || base == 0 {
                if let Ok(f) = s.parse::<f64>() { EvalResult::Value(Value::Number(f)) } else { EvalResult::Value(Value::Void) }
            } else {
                EvalResult::Value(Value::Void)
            }
        }));
    tasks.insert("num_format".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Number(n)) = args.get(0) {
                let base = if let Some(Value::Number(b)) = args.get(1) { *b as u32 } else { 10 };
                if base < 2 || base > 36 { return EvalResult::Value(Value::Void); }
                let val = *n as i64;
                let chars = "0123456789abcdefghijklmnopqrstuvwxyz";
                if val == 0 { return EvalResult::Value(Value::String("0".into())); }
                let mut res = String::new();
                let mut curr = val.abs();
                while curr > 0 {
                    let rem = (curr % (base as i64)) as usize;
                    res.insert(0, chars.chars().nth(rem).unwrap());
                    curr /= base as i64;
                }
                if val < 0 { res.insert(0, '-'); }
                EvalResult::Value(Value::String(res))
            } else { EvalResult::Value(Value::Void) }
        }));

    // --- logic ---
    tasks.insert("logic_and".into(), wrap_native(|args: Vec<Value>, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let mut last = Value::Void;
            for a in args { if matches!(a, Value::Void) { return EvalResult::Value(Value::Void); } last = a.clone(); }
            EvalResult::Value(last)
        }));
    tasks.insert("logic_or".into(), wrap_native(|args: Vec<Value>, _| {
            for a in args { if !matches!(a, Value::Void) { return EvalResult::Value(a.clone()); } }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("logic_eq".into(), wrap_native(|args: Vec<Value>, _| { if let (Some(a), Some(b)) = (args.get(0), args.get(1)) { if hank_equals(a, b) { EvalResult::Value(Value::Number(1.0)) } else { EvalResult::Value(Value::Void) } } else { EvalResult::Value(Value::Void) } }));

    // --- arr ---
    tasks.insert("arr_length".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Array(a)) = args.get(0) { return EvalResult::Value(Value::Number(a.borrow().len() as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_length".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_get".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Array(a)), Some(Value::Number(n))) = (args.get(0), args.get(1)) {
                return EvalResult::Value(a.borrow().get(*n as usize).cloned().unwrap_or(Value::Void));
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_get".into())] })));
                }
            }
            if let Some(other) = args.get(1) {
                if other.get_type() != ValueType::Number {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_get".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_push".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Array(a)), Some(v)) = (args.get(0), args.get(1)) {
                a.borrow_mut().push(v.clone());
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_push".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_pop".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Array(a)) = args.get(0) { return EvalResult::Value(a.borrow_mut().pop().unwrap_or(Value::Void)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_pop".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_each".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if let (Some(Value::Array(a)), Some(Value::Task(t))) = (args.get(0), args.get(1)) {
                let items = a.borrow().clone();
                for (idx, item) in items.iter().enumerate() {
                    let call_args = vec![item.clone(), Value::Number(idx as f64)];
                    let res = ctx.call(&Value::Task(t.clone()), call_args);
                    if let Value::Opaque(op) = &res {
                        if op.label == "__ControlFlow" && op.data.downcast_ref::<String>().map(|s| s == "Break").unwrap_or(false) {
                            break;
                        }
                    }
                    if ctx.is_error(&res) { return EvalResult::Error(res); }
                }
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Array {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Array".into()), Value::String(format!("{:?}", other.get_type())), Value::String("arr_each".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_map".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if let (Some(Value::Array(a)), Some(Value::Task(t))) = (args.get(0), args.get(1)) {
                let items = a.borrow().clone();
                let mut new_items = Vec::new();
                for (idx, item) in items.iter().enumerate() {
                    let call_args = vec![item.clone(), Value::Number(idx as f64)];
                    let res = ctx.call(&Value::Task(t.clone()), call_args);
                    if ctx.is_error(&res) { return EvalResult::Error(res); }
                    new_items.push(res);
                }
                return EvalResult::Value(Value::Array(Arc::new(RefCell::new(new_items))));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_filter".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if let (Some(Value::Array(a)), Some(Value::Task(t))) = (args.get(0), args.get(1)) {
                let items = a.borrow().clone();
                let mut new_items = Vec::new();
                for (idx, item) in items.iter().enumerate() {
                    let call_args = vec![item.clone(), Value::Number(idx as f64)];
                    let res = ctx.call(&Value::Task(t.clone()), call_args);
                    if ctx.is_error(&res) { return EvalResult::Error(res); }
                    if !matches!(res, Value::Void) { new_items.push(item.clone()); }
                }
                return EvalResult::Value(Value::Array(Arc::new(RefCell::new(new_items))));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_indexof".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Array(a)), Some(v)) = (args.get(0), args.get(1)) {
                let items = a.borrow();
                for (idx, item) in items.iter().enumerate() {
                    if hank_equals(item, v) { return EvalResult::Value(Value::Number(idx as f64)); }
                }
                return EvalResult::Value(Value::Number(-1.0));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_shift".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Array(a)) = args.get(0) {
                if a.borrow().is_empty() { return EvalResult::Value(Value::Void); }
                return EvalResult::Value(a.borrow_mut().remove(0));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_unshift".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Array(a)), Some(v)) = (args.get(0), args.get(1)) {
                a.borrow_mut().insert(0, v.clone());
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_slice".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Array(a)) = args.get(0) {
                let start = if let Some(Value::Number(n)) = args.get(1) { *n as i32 } else { 0 };
                let end = if let Some(Value::Number(n)) = args.get(2) { Some(*n as i32) } else { None };
                let items = a.borrow();
                let len = items.len() as i32;
                
                let actual_start = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let actual_end = match end {
                    Some(e) => (if e < 0 { (len + e).max(0) } else { e.min(len) }) as usize,
                    None => len as usize,
                };
                
                if actual_start >= actual_end { return EvalResult::Value(Value::Array(Arc::new(RefCell::new(Vec::new())))); }
                let slice = items[actual_start..actual_end].to_vec();
                return EvalResult::Value(Value::Array(Arc::new(RefCell::new(slice))));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("arr_sort".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if let Some(Value::Array(a)) = args.get(0) {
                let mut items = a.borrow().clone();
                if let Some(Value::Task(t)) = args.get(1) {
                    let mut err = None;
                    items.sort_by(|a, b| {
                        if err.is_some() { return std::cmp::Ordering::Equal; }
                        let res = ctx.call(&Value::Task(t.clone()), vec![a.clone(), b.clone()]);
                        if let Value::Number(n) = res {
                            if n < 0.0 { std::cmp::Ordering::Less }
                            else if n > 0.0 { std::cmp::Ordering::Greater }
                            else { std::cmp::Ordering::Equal }
                        } else if ctx.is_error(&res) {
                            err = Some(res);
                            std::cmp::Ordering::Equal
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    });
                    if let Some(e) = err { return EvalResult::Error(e); }
                } else {
                    items.sort_by(|a, b| {
                        match (a, b) {
                            (Value::Number(n1), Value::Number(n2)) => n1.partial_cmp(n2).unwrap_or(std::cmp::Ordering::Equal),
                            (Value::String(s1), Value::String(s2)) => s1.cmp(s2),
                            _ => std::cmp::Ordering::Equal,
                        }
                    });
                }
                *a.borrow_mut() = items;
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("map_get".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Map(m)), Some(k)) = (args.get(0), args.get(1)) {
                return EvalResult::Value(m.borrow().get(&val_to_string(k)).cloned().unwrap_or(Value::Void));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("map_set".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 3 { return EvalResult::Value(Value::Void); }
            if let (Some(Value::Map(m)), Some(k), Some(v)) = (args.get(0), args.get(1), args.get(2)) {
                m.borrow_mut().insert(val_to_string(k), v.clone());
                return EvalResult::Value(Value::Void);
            }
            if let Some(other) = args.get(0) {
                if other.get_type() != ValueType::Map {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Map".into()), Value::String(format!("{:?}", other.get_type())), Value::String("map_set".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("map_keys".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Map(m)) = args.get(0) {
                let mut keys: Vec<Value> = m.borrow().keys().map(|k| Value::String(k.clone())).collect();
                keys.sort_by(|a, b| if let (Value::String(s1), Value::String(s2)) = (a, b) { s1.cmp(s2) } else { std::cmp::Ordering::Equal });
                EvalResult::Value(Value::Array(Arc::new(RefCell::new(keys))))
            } else { EvalResult::Value(Value::Void) }
        }));
    tasks.insert("map_remove".into(), wrap_native(|args: Vec<Value>, _| {
            if let (Some(Value::Map(m)), Some(k)) = (args.get(0), args.get(1)) {
                m.borrow_mut().remove(&val_to_string(k));
            }
            EvalResult::Value(Value::Void)
        }));

    // --- json ---
    tasks.insert("json_parse".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::String(s)) = args.get(0) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(s) { return EvalResult::Value(map_json_to_hank(data)); }
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("json_stringify".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(v) = args.get(0) {
                if let Some(j) = map_hank_to_json(v) {
                    if let Ok(s) = serde_json::to_string(&j) { return EvalResult::Value(Value::String(s)); }
                }
            }
            EvalResult::Value(Value::Void)
        }));

    // --- err ---
    tasks.insert("err_code".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Error(e)) = args.get(0) { return EvalResult::Value(Value::Number(e.code as i32 as f64)); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err_code".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("err_message".into(), wrap_native(|args: Vec<Value>, ctx: &dyn ExecutionContext| {
            if let Some(Value::Error(e)) = args.get(0) {
                let loc = ctx.get_localization();
                let mut msg = loc.get(&(e.code as i32)).cloned().unwrap_or_else(|| "Unknown Error".into());
                for (i, arg) in e.args.iter().enumerate() {
                    msg = msg.replace(&format!("{{{}}}", i), &val_to_string(arg));
                }
                return EvalResult::Value(Value::String(msg));
            }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err_message".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    tasks.insert("err_args".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(Value::Error(e)) = args.get(0) { return EvalResult::Value(Value::Array(Arc::new(RefCell::new(e.args.clone())))); }
            if let Some(other) = args.get(0) {
                return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Error".into()), Value::String(format!("{:?}", other.get_type())), Value::String("err_args".into())] })));
            }
            EvalResult::Value(Value::Void)
        }));
    // --- type ---
    tasks.insert("type_isVoid".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if matches!(args.get(0).unwrap_or(&Value::Void), Value::Void) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isNumber".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Number(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isString".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::String(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isArray".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Array(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isMap".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Map(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isOpaque".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Opaque(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isTask".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Task(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));
    tasks.insert("type_isError".into(), wrap_native(|args: Vec<Value>, _| EvalResult::Value(if let Some(Value::Error(_)) = args.get(0) { Value::Number(1.0) } else { Value::Void })));

    // --- regex ---
    tasks.insert("regex_parse".into(), wrap_native(|args: Vec<Value>, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let pattern = val_to_string(&args[0]);
            let flags = if args.len() > 1 { val_to_string(&args[1]) } else { "".into() };
            let mut final_pattern = pattern.clone();
            if flags.contains('i') { final_pattern = format!("(?i){}", final_pattern); }
            let re = regex_lite::Regex::new(&final_pattern).ok();
            if let Some(engine) = re {
                EvalResult::Value(Value::Opaque(Arc::new(OpaqueValue {
                    label: "RegExp".into(),
                    data: Box::new(engine),
                })))
            } else {
                EvalResult::Value(Value::Void)
            }
        }));
    tasks.insert("regex_match".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let s = val_to_string(&args[0]);
            match &args[1] {
                Value::Opaque(ov) if ov.label == "RegExp" => {
                    if let Some(re) = ov.data.downcast_ref::<regex_lite::Regex>() {
                        if re.is_match(&s) { return EvalResult::Value(Value::Number(1.0)); }
                    }
                }
                other => if s.contains(&val_to_string(other)) { return EvalResult::Value(Value::Number(1.0)); }
            }
            EvalResult::Value(Value::Void)
        }));

    tasks
}

pub fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let s = n.to_string();
            if s.ends_with(".0") { s[..s.len()-2].to_string() } else { s }
        },
        Value::Void => "Void".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Map(_) => "[Map]".into(),
        Value::Opaque(ov) => format!("[Opaque:{}]", ov.label),
        Value::Task(_) => "[Task]".into(),
        Value::Error(e) => format!("[Error:{:?}]", e.code),
    }
}

fn map_json_to_hank(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Void,
        serde_json::Value::Bool(b) => if b { Value::Number(1.0) } else { Value::Void },
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => Value::Array(Arc::new(RefCell::new(a.into_iter().map(map_json_to_hank).collect()))),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, val) in o { map.insert(k, map_json_to_hank(val)); }
            Value::Map(Arc::new(RefCell::new(map)))
        }
    }
}

fn map_hank_to_json(v: &Value) -> Option<serde_json::Value> {
    match v {
        Value::Void => Some(serde_json::Value::Null),
        Value::Number(n) => Some(serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())),
        Value::String(s) => Some(serde_json::Value::String(s.clone())),
        Value::Array(a) => {
            let mut items = vec![];
            for i in a.borrow().iter() {
                items.push(map_hank_to_json(i)?);
            }
            Some(serde_json::Value::Array(items))
        },
        Value::Map(o) => {
            let mut map = serde_json::Map::new();
            for (k, val) in o.borrow().iter() {
                map.insert(k.clone(), map_hank_to_json(val)?);
            }
            Some(serde_json::Value::Object(map))
        },
        Value::Opaque(_) => None,
        _ => Some(serde_json::Value::Null),
    }
}
