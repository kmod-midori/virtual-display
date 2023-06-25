use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_path = PathBuf::from(std::env::var("OUT_DIR")?);

    let library = vcpkg::find_package("ffmpeg").unwrap();

    println!("cargo:rerun-if-changed=src/wrapper.h");

    println!("cargo:rustc-link-lib=mfuuid");
    println!("cargo:rustc-link-lib=mfplat");
    println!("cargo:rustc-link-lib=strmiids");
    println!("cargo:rustc-link-lib=user32");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let mut bindings_builder = bindgen::Builder::default()
        .header("src/wrapper.h")
        .allowlist_function("av_.*")
        .allowlist_function("avcodec_.*")
        .allowlist_function("av_image_.*")
        .allowlist_var("FF_PROFILE.*")
        .allowlist_var("AV_.*")
        .allowlist_var("AVERROR_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks));

    for include_path in &library.include_paths {
        bindings_builder = bindings_builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = bindings_builder.generate()?;

    bindings.write_to_file(out_path.join("bindings.rs"))?;

    Ok(())
}
