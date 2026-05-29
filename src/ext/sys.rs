use crate::types::{Value, NativeFunc, HankExtension, EvalResult, Arc};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct SysExtension;

impl HankExtension for SysExtension {
    fn name(&self) -> &str { "SysExtension" }
    fn get_modules(&self) -> HashMap<String, HashMap<String, NativeFunc>> {
        let mut modules = HashMap::new();

        // --- host ---
        let mut host_mod = HashMap::new();
        host_mod.insert("cwd".into(), (|_, _| {
            let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().to_string();
            EvalResult::Value(Value::String(cwd))
        }) as NativeFunc);
        host_mod.insert("pid".into(), (|_, _| {
            EvalResult::Value(Value::Number(std::process::id() as f64))
        }) as NativeFunc);
        modules.insert("host".into(), host_mod);

        // --- os ---
        let mut os_mod = HashMap::new();
        os_mod.insert("type".into(), (|_, _| {
            EvalResult::Value(Value::String(std::env::consts::OS.to_string()))
        }) as NativeFunc);
        os_mod.insert("arch".into(), (|_, _| {
            EvalResult::Value(Value::String(std::env::consts::ARCH.to_string()))
        }) as NativeFunc);
        modules.insert("os".into(), os_mod);

        // --- fs ---
        let mut fs_mod = HashMap::new();
        fs_mod.insert("exists".into(), (|args, _| {
            if let Some(Value::String(path)) = args.get(0) {
                if std::path::Path::new(path).exists() { return EvalResult::Value(Value::Number(1.0)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("read".into(), (|args, _| {
            if let Some(Value::String(path)) = args.get(0) {
                if let Ok(content) = std::fs::read_to_string(path) { return EvalResult::Value(Value::String(content)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("write".into(), (|args, _| {
            if let (Some(Value::String(path)), Some(Value::String(content))) = (args.get(0), args.get(1)) {
                if std::fs::write(path, content).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("deleteFile".into(), (|args, _| {
            if let Some(Value::String(path)) = args.get(0) {
                if std::fs::remove_file(path).is_ok() { return EvalResult::Value(Value::Number(1.0)); }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        fs_mod.insert("stat".into(), (|args, _| {
            if let Some(Value::String(path)) = args.get(0) {
                if let Ok(meta) = std::fs::metadata(path) {
                    let mut map = HashMap::new();
                    map.insert("size".into(), Value::Number(meta.len() as f64));
                    map.insert("isDir".into(), if meta.is_dir() { Value::Number(1.0) } else { Value::Void });
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            map.insert("mtime".into(), Value::Number(dur.as_millis() as f64));
                        }
                    }
                    return EvalResult::Value(Value::Object(Arc::new(RefCell::new(map))));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        modules.insert("fs".into(), fs_mod);

        // --- proc ---
        let mut proc_mod = HashMap::new();
        proc_mod.insert("run".into(), (|args, _| {
            if let Some(Value::String(cmd)) = args.get(0) {
                let mut command = std::process::Command::new(cmd);
                if let Some(Value::Array(a)) = args.get(1) {
                    for arg in a.borrow().iter() {
                        command.arg(val_to_string(arg));
                    }
                }
                if let Ok(output) = command.output() {
                    let mut map = HashMap::new();
                    map.insert("code".into(), Value::Number(output.status.code().unwrap_or(1) as f64));
                    map.insert("stdout".into(), Value::String(String::from_utf8_lossy(&output.stdout).to_string()));
                    map.insert("stderr".into(), Value::String(String::from_utf8_lossy(&output.stderr).to_string()));
                    return EvalResult::Value(Value::Object(Arc::new(RefCell::new(map))));
                }
            }
            EvalResult::Value(Value::Void)
        }) as NativeFunc);
        modules.insert("proc".into(), proc_mod);

        modules
    }
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Void => "Void".into(),
        Value::Array(_) => "[Array]".into(),
        Value::Object(_) => "{Object}".into(),
        Value::Opaque(ov) => format!("[Opaque:{}]", ov.label),
        Value::Task(_) => "[Task]".into(),
    }
}
