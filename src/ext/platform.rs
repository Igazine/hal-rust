use crate::types::{Value, NativeFunc, HankExtension, EvalResult, HankError, ValueType, ErrorValue, Arc, ExecutionContext};
use std::collections::HashMap;

const SAFE_INT_MAX: f64 = 9007199254740991.0;

fn check_safe_int(n: f64, task_name: &str) -> Result<i64, Value> {
    if n.abs() > SAFE_INT_MAX || !n.is_finite() {
        return Err(Value::Error(Arc::new(ErrorValue {
            code: HankError::BitwiseOutOfBounds,
            args: vec![Value::Number(n), Value::String(task_name.into())],
        })));
    }
    Ok(n as i64)
}

fn from_safe_int(n: i64, task_name: &str) -> Result<f64, Value> {
    let f = n as f64;
    if f.abs() > SAFE_INT_MAX {
        return Err(Value::Error(Arc::new(ErrorValue {
            code: HankError::BitwiseOutOfBounds,
            args: vec![Value::Number(f), Value::String(task_name.into())],
        })));
    }
    Ok(f)
}

fn wrap_native<F>(f: F) -> NativeFunc 
where F: for<'a> Fn(Vec<Value>, &'a dyn ExecutionContext) -> EvalResult + 'static 
{
    Arc::new(f)
}

pub struct PlatformExtension;

impl HankExtension for PlatformExtension {
    fn name(&self) -> &str { "PlatformExtension" }
    fn get_tasks(&self) -> HashMap<String, NativeFunc> {
        let mut tasks = HashMap::new();

        tasks.insert("bin_and".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_and".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_and".into())] })))
            };
            let ia = match check_safe_int(a, "bin_and") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin_and") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia & ib, "bin_and") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks.insert("bin_or".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_or".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_or".into())] })))
            };
            let ia = match check_safe_int(a, "bin_or") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin_or") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia | ib, "bin_or") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks.insert("bin_xor".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_xor".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_xor".into())] })))
            };
            let ia = match check_safe_int(a, "bin_xor") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b, "bin_xor") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia ^ ib, "bin_xor") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks.insert("bin_not".into(), wrap_native(|args: Vec<Value>, _| {
            if args.is_empty() { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_not".into())] })))
            };
            let ia = match check_safe_int(a, "bin_not") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(!ia, "bin_not") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks.insert("bin_shiftL".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_shiftL".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_shiftL".into())] })))
            };
            let ia = match check_safe_int(a, "bin_shiftL") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia << (b as u32), "bin_shiftL") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks.insert("bin_shiftR".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let a = match args.get(0).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_shiftR".into())] })))
            };
            let b = match args.get(1).unwrap() {
                Value::Number(n) => *n,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("Number".into()), Value::String(format!("{:?}", other.get_type())), Value::String("bin_shiftR".into())] })))
            };
            let ia = match check_safe_int(a, "bin_shiftR") { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia >> (b as i32), "bin_shiftR") {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }));

        tasks
    }
}
