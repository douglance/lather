use super::tightening;

#[test]
fn accepts_stricter_flags_without_scope_changes_or_weaker_options() {
    let initial =
        r#"{"compilerOptions":{"strict":false,"noImplicitReturns":true},"include":["src"]}"#;
    assert!(tightening(
        initial,
        r#"{"compilerOptions":{"strict":true,"noImplicitReturns":true},"include":["src"]}"#
    ));
    assert!(!tightening(
        initial,
        r#"{"compilerOptions":{"strict":true,"noImplicitReturns":true},"include":["src/subset"]}"#
    ));
    assert!(!tightening(
        initial,
        r#"{"compilerOptions":{"strict":true,"noImplicitReturns":false},"include":["src"]}"#
    ));
}
