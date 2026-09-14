use std::process::Command;

fn main() {
    // onnxruntime
    println!("cargo:rustc-link-search=native=thirdparty/onnxruntime");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");

    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/libs");

    // 注入 git commit，供 `server.build_info` 指标使用；无 git 环境时回退 unknown。
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|commit| !commit.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=MEMORY_SEEK_GIT_COMMIT={commit}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
}
