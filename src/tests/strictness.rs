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

#[test]
fn measures_matt_pocock_tsconfig_cheat_sheet_flags() {
    // Matt-authored: https://www.totaltypescript.com/tsconfig-cheat-sheet
    // isolatedModules and verbatimModuleSyntax sit after noImplicitOverride so
    // suggest() offers them before flags Matt calls too noisy by default.
    let before_noisy: Vec<String> = [
        "noImplicitAny",
        "noImplicitThis",
        "strictNullChecks",
        "strictFunctionTypes",
        "strictBindCallApply",
        "strictPropertyInitialization",
        "strictBuiltinIteratorReturn",
        "alwaysStrict",
        "useUnknownInCatchVariables",
        "noUncheckedIndexedAccess",
        "noImplicitOverride",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(super::next_option(&before_noisy), Some("isolatedModules"));
    assert!(super::FLAGS.contains(&"isolatedModules"));
    assert!(super::FLAGS.contains(&"verbatimModuleSyntax"));
    let isolated_idx = super::FLAGS
        .iter()
        .position(|&flag| flag == "isolatedModules")
        .unwrap();
    let verbatim_idx = super::FLAGS
        .iter()
        .position(|&flag| flag == "verbatimModuleSyntax")
        .unwrap();
    let override_idx = super::FLAGS
        .iter()
        .position(|&flag| flag == "noImplicitOverride")
        .unwrap();
    let fallthrough_idx = super::FLAGS
        .iter()
        .position(|&flag| flag == "noFallthroughCasesInSwitch")
        .unwrap();
    assert!(override_idx < isolated_idx && isolated_idx < verbatim_idx);
    assert!(verbatim_idx < fallthrough_idx);
    assert!(super::tightening(
        r#"{"compilerOptions":{"strict":true},"include":["src"]}"#,
        r#"{"compilerOptions":{"strict":true,"isolatedModules":true,"verbatimModuleSyntax":true},"include":["src"]}"#,
    ));
}
