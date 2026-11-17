use super::measure;

#[test]
fn locates_production_exit_sites_without_counting_test_code_or_strings() {
    let source = r#"fn first(value: Option<i32>) -> i32 {
    value.expect("explicit")
}
#[cfg(test)]
mod tests { fn helper() { panic!("test setup"); } }
#[tokio::test]
async fn test_case() { None::<i32>.unwrap(); }
fn second() { unreachable!("unreachable"); }
const MESSAGE: &str = "foo.unwrap()";
"#;
    let measured = measure(
        [Ok(("fixture.rs".into(), source.into()))].into_iter(),
        500,
        true,
    )
    .unwrap();
    assert!(measured.issues.is_empty());
    assert_eq!(measured.lines, 9);
    let actual: Vec<_> = measured
        .unchecked_exits
        .iter()
        .map(|item| (item.path.as_str(), item.line, item.construct.as_str()))
        .collect();
    assert_eq!(
        actual,
        [
            ("fixture.rs", 2, "expect"),
            ("fixture.rs", 8, "unreachable")
        ]
    );
}
