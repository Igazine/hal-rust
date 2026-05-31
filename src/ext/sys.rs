use crate::types::{Value, NativeFunc, HankExtension, EvalResult, Arc, ValueType, ErrorValue, HankError, OpaqueValue, ExecutionContext};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct SysExtension;

fn wrap_native<F>(f: F) -> NativeFunc 
where F: for<'a> Fn(Vec<Value>, &'a dyn ExecutionContext) -> EvalResult + 'static 
{
    Arc::new(f)
}

impl HankExtension for SysExtension {
    fn name(&self) -> &str { "SysExtension" }
    fn get_tasks(&self) -> HashMap<String, NativeFunc> {
        let mut tasks = HashMap::new();

        // --- host ---
        tasks.insert("host_cwd".into(), wrap_native(|_, _| {
            let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().to_string();
            EvalResult::Value(Value::String(cwd))
        }));
        tasks.insert("host_pid".into(), wrap_native(|_, _| {
            EvalResult::Value(Value::Number(std::process::id() as f64))
        }));
        tasks.insert("host_isRoot".into(), wrap_native(|_, _| {
            EvalResult::Value(Value::Void)
        }));

        // --- os ---
        tasks.insert("os_type".into(), wrap_native(|_, _| {
            EvalResult::Value(Value::String(std::env::consts::OS.to_string()))
        }));
        tasks.insert("os_arch".into(), wrap_native(|_, _| {
            EvalResult::Value(Value::String(std::env::consts::ARCH.to_string()))
        }));
        tasks.insert("os_memory".into(), wrap_native(|_, _| {
            let mut fields = HashMap::new();
            fields.insert("total".into(), Value::Number(0.0));
            fields.insert("free".into(), Value::Number(0.0));
            fields.insert("used".into(), Value::Number(0.0));
            EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))))
        }));
        tasks.insert("os_cpu".into(), wrap_native(|_, _| {
            EvalResult::Value(Value::Number(0.0))
        }));

        // --- fs ---
        tasks.insert("fs_exists".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if std::path::Path::new(&path).exists() { return EvalResult::Value(Value::Number(1.0)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs_exists".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
        tasks.insert("fs_read".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if let Ok(content) = std::fs::read_to_string(path) { return EvalResult::Value(Value::String(content)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs_read".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
        tasks.insert("fs_write".into(), wrap_native(|args: Vec<Value>, _| {
            if args.len() < 2 { return EvalResult::Value(Value::Void); }
            let path = match args.get(0).unwrap() {
                Value::String(s) => s,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("fs_write".into())] })))
            };
            let content = match args.get(1).unwrap() {
                Value::String(s) => s,
                other => return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", other.get_type())), Value::String("fs_write".into())] })))
            };
            if std::fs::write(path, content).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
            EvalResult::Value(Value::Void)
        }));
        tasks.insert("fs_deleteFile".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if std::fs::remove_file(path).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs_deleteFile".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));
        tasks.insert("fs_stat".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(path) = val {
                    if let Ok(meta) = std::fs::metadata(path) {
                        let mut fields = HashMap::new();
                        fields.insert("size".into(), Value::Number(meta.len() as f64));
                        fields.insert("isDir".into(), if meta.is_dir() { Value::Number(1.0) } else { Value::Void });
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                                fields.insert("mtime".into(), Value::Number(dur.as_millis() as f64));
                            }
                        }
                        return EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))));
                    }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("fs_stat".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));

        // --- proc ---
        tasks.insert("proc_run".into(), wrap_native(|args: Vec<Value>, _| {
            if let Some(val) = args.get(0) {
                if let Value::String(cmd) = val {
                    let mut command = std::process::Command::new(cmd);
                    if let Some(Value::Array(a)) = args.get(1) {
                        for arg in a.borrow().iter() {
                            let s = match arg {
                                Value::String(s) => s.clone(),
                                Value::Number(n) => n.to_string(),
                                _ => "Void".into(),
                            };
                            command.arg(s);
                        }
                    }
                    if let Ok(output) = command.output() {
                        let mut fields = HashMap::new();
                        fields.insert("code".into(), Value::Number(output.status.code().unwrap_or(1) as f64));
                        fields.insert("stdout".into(), Value::String(String::from_utf8_lossy(&output.stdout).to_string()));
                        fields.insert("stderr".into(), Value::String(String::from_utf8_lossy(&output.stderr).to_string()));
                        return EvalResult::Value(Value::Map(Arc::new(RefCell::new(fields))));
                    }
                } else {
                    return EvalResult::Error(Value::Error(Arc::new(ErrorValue { code: HankError::TypeMismatch, args: vec![Value::String("String".into()), Value::String(format!("{:?}", val.get_type())), Value::String("proc_run".into())] })));
                }
            }
            EvalResult::Value(Value::Void)
        }));

        tasks
    }
}
