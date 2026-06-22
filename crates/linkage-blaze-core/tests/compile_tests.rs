#[test]
fn ui_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui_pass/*.rs");
}

#[test]
fn ui_fail() {
    use std::{fs, path::PathBuf, process::Command};

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ui_fail_dir = manifest_dir.join("tests/ui_fail");
    let scratch_dir = manifest_dir.join("../../target/ui-fail-smoke/linkage-blaze-core");
    let src_dir = scratch_dir.join("src");

    fs::create_dir_all(&src_dir).expect("create ui-fail smoke src dir");
    fs::write(
        scratch_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"linkage-blaze-core-ui-fail-smoke\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nlinkage-blaze-core = {{ path = {:?} }}\n",
            manifest_dir
        ),
    )
    .expect("write ui-fail smoke manifest");

    let mut fixtures = fs::read_dir(&ui_fail_dir)
        .expect("read ui-fail dir")
        .map(|entry| entry.expect("read ui-fail entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    fixtures.sort();

    for fixture in fixtures {
        let source = fs::read_to_string(&fixture).expect("read ui-fail fixture");
        fs::write(src_dir.join("main.rs"), source).expect("write ui-fail smoke main");

        let output = Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(scratch_dir.join("Cargo.toml"))
            .output()
            .expect("run cargo check for ui-fail fixture");

        assert!(
            !output.status.success(),
            "expected fixture to fail compilation: {}",
            fixture.display()
        );
    }
}
