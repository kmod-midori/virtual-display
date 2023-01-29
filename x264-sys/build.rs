use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/wrapper.h");

    let library = vcpkg::find_package("x264").unwrap();

    let mut binding_builder = bindgen::Builder::default().header("src/wrapper.h");
    for include_path in &library.include_paths {
        binding_builder = binding_builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = binding_builder
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    let mut cc_build = cc::Build::new();
    for include_path in &library.include_paths {
        cc_build.include(include_path);
    }
    cc_build.file("src/wrapper.c").compile("x264-wrapper");
}
