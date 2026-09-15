use std::path::Path;
use std::process::Command;

fn main() {
    // onnxruntime
    println!("cargo:rustc-link-search=native=thirdparty/onnxruntime");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");

    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/libs");

    // 注入 git commit
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .filter(|sha| !sha.is_empty())
        .map(|sha| sha.chars().take(7).collect::<String>())
        .or_else(git_short_commit)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=MEMORY_SEEK_GIT_COMMIT={commit}");

    // `rerun-if-changed` 必须指向"提交时会变"的位置:
    // `.git/HEAD` 只存分支引用(`ref: refs/heads/x`), 提交时内容不变;
    // 随提交变化的是分支 ref / packed-refs / reflog。
    // 仅对存在的路径声明, 避免缺失路径导致每次构建都重跑脚本。
    for path in [
        "../.git/HEAD",
        "../.git/refs/heads",
        "../.git/packed-refs",
        "../.git/logs/HEAD",
    ] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

/// 取当前 HEAD 的短 sha; 无 git 或命令失败时返回 `None`。
fn git_short_commit() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|commit| !commit.is_empty())
}
