use std::error::Error;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or("CARGO_MANIFEST_DIR is not set")?,
    );
    let repository_readme = manifest_directory.join("../../README.md");
    let packaged_readme = manifest_directory.join("README.md");
    let readme = if repository_readme.is_file() {
        repository_readme
    } else {
        packaged_readme
    };

    println!("cargo:rerun-if-changed={}", readme.display());
    let output_readme = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("README.md");
    fs::copy(&readme, output_readme)?;
    Ok(())
}
