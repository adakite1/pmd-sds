use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::copy("arm9.ld", out.join("arm9.ld")).unwrap();
    println!("cargo::rustc-link-search={}", out.display());
    println!("cargo::rerun-if-changed=arm9.ld");
    println!("cargo::rerun-if-changed=build.rs");
}

