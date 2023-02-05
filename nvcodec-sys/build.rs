use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/wrapper.h");

    let library = vcpkg::find_package("ffnvcodec").unwrap();

    let mut binding_builder = bindgen::Builder::default().header("src/wrapper.h");
    for include_path in &library.include_paths {
        binding_builder = binding_builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = binding_builder
        .blocklist_file("windows.h")
        .derive_debug(true)
        .derive_eq(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
