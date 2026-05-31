use hank::types::{Value, ValueType, Expr, Resource, Arc};
use hank::runner::Runner;
use hank::stdlib;
use hank::ext::platform::PlatformExtension;
use hank::ext::sys::SysExtension;
use std::collections::HashMap;
use std::cell::RefCell;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct FileResource {
    id: String,
    content: RefCell<Option<String>>,
    ast: RefCell<Option<Expr>>,
}

impl FileResource {
    pub fn new(id: String) -> Self {
        Self {
            id,
            content: RefCell::new(None),
            ast: RefCell::new(None),
        }
    }
}

impl Resource for FileResource {
    fn id(&self) -> &str { &self.id }
    fn content(&self) -> Option<String> { self.content.borrow().clone() }
    fn ast(&self) -> Option<Expr> { self.ast.borrow().clone() }
    fn set_ast(&self, ast: Expr) { *self.ast.borrow_mut() = Some(ast); }
    fn load(&self) -> Result<(), String> {
        let path = Path::new(&self.id);
        if path.exists() {
            let s = fs::read_to_string(path).map_err(|e| e.to_string())?;
            *self.content.borrow_mut() = Some(s);
            Ok(())
        } else {
            Err(format!("File not found: {}", self.id))
        }
    }
    fn resolve(&self, id: &str) -> Result<Box<dyn Resource>, String> {
        let parent = Path::new(&self.id).parent().unwrap_or(Path::new("."));
        let new_path = parent.join(id);
        let id_str = if new_path.extension().is_none() {
            new_path.with_extension("hank").to_string_lossy().to_string()
        } else {
            new_path.to_string_lossy().to_string()
        };
        Ok(Box::new(FileResource::new(id_str)))
    }
}

fn create_runner() -> Runner {
    let runner = Runner::new();

    // 0. Localization
    let mut loc = HashMap::new();
    loc.insert(4001, "Target is not a function: {0}".into());
    loc.insert(4002, "Too many arguments".into());
    loc.insert(4007, "Type Mismatch: Expected {0}, got {1} in {2}".into());
    loc.insert(4005, "Value exceeds safe integer bounds: {0} in {1}".into());
    loc.insert(4008, "Instruction Limit Exceeded: Script reached the maximum allowed AST evaluations ({0})".into());
    runner.register_localization(loc);

    // 1. Register StdLib (Pure)
    runner.register_extension(Box::new(stdlib::StdLib::new()));

    // 2. Register Extensions (Batteries included, but disconnected)
    runner.register_extension(Box::new(PlatformExtension));
    runner.register_extension(Box::new(SysExtension));

    runner
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let current_dir = std::env::current_dir().unwrap();
    // Submodule is at vendor/hank relative to workspace root, but we are in hank-rust/examples/demo
    let root = current_dir.join("../../vendor/hank");

    if args.len() < 2 {
        run_conformance(&root);
        return;
    }

    let runner = create_runner();
    let script_path = &args[1];
    let res = Arc::new(FileResource::new(script_path.clone()));

    let mut hank_args = vec![];
    for arg in &args[2..] {
        hank_args.push(Value::String(arg.clone()));
    }

    match runner.run(res, hank_args) {
        Ok(val) => {
            if let Value::Error(e) = val {
                let loc = runner.localization.borrow();
                let mut msg = loc.get(&(e.code as i32)).cloned().unwrap_or_else(|| "Unknown Error".into());
                for (i, arg) in e.args.iter().enumerate() {
                    msg = msg.replace(&format!("{{{}}}", i), &stdlib::val_to_string(arg));
                }
                eprintln!("Runtime Error {}: {}", e.code as i32, msg);
                std::process::exit(1);
            }
            if let Value::Number(n) = val {
                std::process::exit(n as i32);
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn run_conformance(root: &Path) {
    let tests = [
        "test/conformance/01_literals.hank",
        "test/conformance/02_gates.hank",
        "test/conformance/03_scoping.hank",
        "test/conformance/04_hoisting.hank",
        "test/conformance/05_params.hank",
        "test/conformance/06_macros.hank",
        "test/conformance/07_returns.hank",
        "test/conformance/08_host_args.hank",
        "test/conformance/09_deep_nesting.hank",
        "test/conformance/10_edge_cases.hank",
        "test/conformance/11_regex_parse.hank",
        "test/conformance/12_data_advanced.hank",
        "test/conformance/13_logic_module.hank",
        "test/conformance/15_logic_eq.hank",
        "test/conformance/16_chained_assign.hank",
        "test/conformance/17_num_module.hank",
        "test/conformance/18_runtime_module.hank",
        "test/conformance/19_error_handling.hank",
        "test/conformance/20_grammar_hardening.hank",
        "test/conformance/21_data_functional.hank",
        "test/conformance/22_instruction_limit.hank",
    ];

    for t in &tests {
        println!("--- Running: {} ---", t);
        let mut runner = create_runner();
        if t.contains("22_instruction_limit") {
            runner.max_instructions = 1000;
        }
        let path = root.join(t);
        let abs_path = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => { println!("Test not found: {}", path.display()); continue; }
        };
        let res = Arc::new(FileResource::new(abs_path.to_string_lossy().to_string()));
        let mut args = vec![];
        if t.ends_with("08_host_args.hank") {
            args.push(Value::String("Tamas".into()));
        }
        match runner.run(res, args) {
            Ok(val) => {
                if let Value::Error(e) = val {
                    let loc = runner.localization.borrow();
                    let mut msg = loc.get(&(e.code as i32)).cloned().unwrap_or_else(|| "Unknown Error".into());
                    for (i, arg) in e.args.iter().enumerate() {
                        msg = msg.replace(&format!("{{{}}}", i), &stdlib::val_to_string(arg));
                    }
                    println!("Test Runtime Error {}: {}", e.code as i32, msg);
                }
            },
            Err(e) => {
                println!("Test Failed: {}", e);
            },
        }
        println!("--------------------\n");
    }

    // Run Extension Tests
    let ext_tests = [
        "test/extensions/sys.hank",
        "test/extensions/platform_bin.hank",
    ];

    for t in &ext_tests {
        println!("--- Running Extension Test: {} ---", t);
        let runner = create_runner();
        let path = root.join(t);
        let abs_path = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => { println!("Test not found: {}", path.display()); continue; }
        };
        let res = Arc::new(FileResource::new(abs_path.to_string_lossy().to_string()));
        match runner.run(res, vec![]) {
            Ok(val) => {
                if let Value::Error(e) = val {
                    let loc = runner.localization.borrow();
                    let mut msg = loc.get(&(e.code as i32)).cloned().unwrap_or_else(|| "Unknown Error".into());
                    for (i, arg) in e.args.iter().enumerate() {
                        msg = msg.replace(&format!("{{{}}}", i), &stdlib::val_to_string(arg));
                    }
                    println!("Extension Runtime Error {}: {}", e.code as i32, msg);
                }
            },
            Err(e) => {
                println!("Extension Test Failed: {}", e);
            },
        }
        println!("--------------------\n");
    }
}
