use std::env;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let ext_lib_path = Path::new(&manifest_dir).join("ext_lib");
    let target_so = ext_lib_path.join("libducklingffi.so");

    if !target_so.exists() {
        panic!(
            "ext_lib/libducklingffi.so not found. Run `make haskell` first to build it."
        );
    }

    println!("cargo:rustc-link-search=native={}", ext_lib_path.display());
    println!("cargo:rustc-link-lib=dylib=ducklingffi");

    let ext_lib_abs = ext_lib_path
        .canonicalize()
        .unwrap_or_else(|_| ext_lib_path.clone());

    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", ext_lib_abs.display());
    println!("cargo:lib_dir={}", ext_lib_abs.display());

    println!("cargo:rerun-if-changed=ext_lib/libducklingffi.so");
}
