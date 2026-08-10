use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let gzip_dir = PathBuf::from(manifest_dir)
        .join("rarezip")
        .join("gzip");

    //link library
    println!("cargo:rustc-link-search=native={}", gzip_dir.display());
    println!("cargo:rustc-link-lib=static=rarezip");
    println!("cargo:rerun-if-changed=build.rs");
}
