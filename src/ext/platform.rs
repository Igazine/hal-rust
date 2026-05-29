use crate::types::{Value, NativeFunc, HankExtension, EvalResult, HankError};
use crate::error_registry::HankErrorRegistry;
use std::collections::HashMap;

const SAFE_INT_MAX: f64 = 9007199254740991.0;

fn check_safe_int(n: f64) -> Result<i64, crate::types::HankErrorValue> {
    if n.abs() > SAFE_INT_MAX || !n.is_finite() {
        return Err(HankErrorRegistry::create(
            HankError::BitwiseOutOfBounds,
            vec![n.to_string()],
            None, None, None
        ));
    }
    Ok(n as i64)
}

fn from_safe_int(n: i64) -> Result<f64, crate::types::HankErrorValue> {
    let f = n as f64;
    if f.abs() > SAFE_INT_MAX {
        return Err(HankErrorRegistry::create(
            HankError::BitwiseOutOfBounds,
            vec![f.to_string()],
            None, None, None
        ));
    }
    Ok(f)
}

pub struct PlatformExtension;

impl HankExtension for PlatformExtension {
    fn name(&self) -> &str { "PlatformExtension" }
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>> {
        let mut modules = HashMap::new();
        let mut bin_mod = HashMap::new();

        bin_mod.insert("and".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let b = if let Some(Value::Number(n)) = args.get(1) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia & ib) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("or".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let b = if let Some(Value::Number(n)) = args.get(1) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia | ib) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("xor".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let b = if let Some(Value::Number(n)) = args.get(1) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            let ib = match check_safe_int(b) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia ^ ib) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("not".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(!ia) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("shiftL".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let b = if let Some(Value::Number(n)) = args.get(1) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia << (b as u32)) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        bin_mod.insert("shiftR".into(), (|args, _| {
            let a = if let Some(Value::Number(n)) = args.get(0) { *n } else { 0.0 };
            let b = if let Some(Value::Number(n)) = args.get(1) { *n } else { 0.0 };
            let ia = match check_safe_int(a) { Ok(i) => i, Err(e) => return EvalResult::Error(e) };
            match from_safe_int(ia >> (b as u32)) {
                Ok(f) => EvalResult::Value(Value::Number(f)),
                Err(e) => EvalResult::Error(e)
            }
        }) as NativeFunc);

        modules.insert("bin".into(), bin_mod);
        modules
    }
}
