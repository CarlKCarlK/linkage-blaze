#[test]
fn ui_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui_pass/*.rs");
}

#[test]
fn ui_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui_fail/*.rs");
}
