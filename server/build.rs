fn main() {
    // onnxruntime
    println!("cargo:rustc-link-search=native=thirdparty/onnxruntime");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");

    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/libs");
}
