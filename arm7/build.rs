use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::copy("arm7.ld", out.join("arm7.ld")).unwrap();
    println!("cargo::rustc-link-search={}", out.display());
    println!("cargo::rerun-if-changed=arm7.ld");
    println!("cargo::rerun-if-changed=build.rs");
}

