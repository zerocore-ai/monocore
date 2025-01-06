use std::{env, path::Path};

fn main() {
    // Get the manifest directory (where Cargo.toml lives)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_dir = Path::new(&manifest_dir).parent().unwrap().join("build");

    // Print current directory and build directory for debugging
    println!(
        "cargo:warning=Current dir: {:?}",
        std::env::current_dir().unwrap()
    );
    println!("cargo:warning=Build dir: {:?}", build_dir);

    // Add build directory as first search path
    println!("cargo:rustc-link-search=native={}", build_dir.display());

    // Add system paths as fallback
    println!("cargo:rustc-link-search=native=/usr/local/lib");

    // Add user-specific library as fallback
    println!(
        "cargo:rustc-link-search=native={}/.local/lib",
        env::var("HOME").unwrap()
    );

    // Link against libkrun library
    println!("cargo:rustc-link-lib=dylib=krun");

    // Force rebuild if the library changes
    println!(
        "cargo:rerun-if-changed={}",
        build_dir.join("libkrun.dylib").display()
    );

    println!(
        "cargo:rerun-if-changed={}",
        build_dir.join("libkrun.so").display()
    );
}
