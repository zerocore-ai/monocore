fn main() {
    // Add search paths for libkrunfw and libkrun dynamic libraries
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-search=/usr/local/lib64");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=/usr/local/lib");
    }
}
