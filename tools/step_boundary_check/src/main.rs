use std::fs;
use std::path::{Path, PathBuf};

use step_boundary_check::{check_source, Violation};

fn main() {
    let root = std::env::args()
        .skip_while(|a| a != "--root")
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut violations = Vec::new();
    collect_rs(&root, &mut violations);

    if violations.is_empty() {
        println!("step_boundary_check: OK, Step 与错误调用边界检查通过");
        return;
    }

    for v in &violations {
        eprintln!("{}:{}:{}: {}", v.file, v.line, v.column, v.message);
    }
    eprintln!("\nstep_boundary_check: 发现 {} 处违规", violations.len());
    std::process::exit(1);
}

fn collect_rs(dir: &Path, out: &mut Vec<Violation>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if matches!(
                name.as_str(),
                "target" | ".git" | "bindings" | "node_modules" | "models"
            ) {
                continue;
            }
            collect_rs(&path, out);
        } else if name.ends_with(".rs") {
            if let Ok(source) = fs::read_to_string(&path) {
                out.extend(check_source(&source, &path.to_string_lossy()));
            }
        }
    }
}
