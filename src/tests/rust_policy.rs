use super::{forbids_at_crate_root, forbids_in_manifest};

#[test]
fn only_unconditional_root_forbids_count() {
    for source in [
        "#![forbid(unsafe_code)]",
        "#![forbid(unused, unsafe_code, reason = \"Keep this crate safe\")]",
    ] {
        assert!(forbids_at_crate_root(&syn::parse_file(source).unwrap()));
    }

    for source in [
        "#![deny(unsafe_code)]",
        "#![cfg_attr(feature = \"strict\", forbid(unsafe_code))]",
        "mod inner { #![forbid(unsafe_code)] }",
        "#[forbid(unsafe_code)] fn f() {}",
        "// #![forbid(unsafe_code)]",
    ] {
        assert!(!forbids_at_crate_root(&syn::parse_file(source).unwrap()));
    }
}

#[test]
fn cargo_lints_support_string_and_table_levels() {
    for (source, expected) in [
        ("[rust]\nunsafe_code = \"forbid\"", true),
        (
            "[rust]\nunsafe_code = { level = \"forbid\", priority = 1 }",
            true,
        ),
        ("[rust]\nunsafe_code = \"deny\"", false),
        ("[clippy]\nunsafe_code = \"forbid\"", false),
    ] {
        let lints = toml::from_str(source).unwrap();
        assert_eq!(forbids_in_manifest(Some(&lints)), expected);
    }
}

#[test]
fn lint_levels_respect_root_overrides() {
    let root = syn::parse_file("#![allow(unused_must_use)]").unwrap();
    assert!(!super::strict_at_root(
        &root,
        Some("deny"),
        "unused_must_use"
    ));
    assert!(super::strict_at_root(
        &root,
        Some("forbid"),
        "unused_must_use"
    ));
    let root = syn::parse_file("#![deny(unused_must_use)]").unwrap();
    assert!(super::strict_at_root(&root, None, "unused_must_use"));
}
