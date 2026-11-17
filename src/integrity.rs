use crate::model::{Result, WithCode};
use std::collections::{BTreeMap, BTreeSet};

pub fn test_path(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            "test" | "tests" | "testdata" | "benches" | "__tests__"
        )
    }) || path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .is_some_and(|(name, _)| name.ends_with(".test") || name.ends_with(".spec"))
}

pub fn policy_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with(".github/")
        || path.split('/').any(|part| part == ".cargo")
        || name.starts_with(".eslintrc")
        || name.starts_with(".prettier")
        || name.starts_with("eslint.config.")
        || name.starts_with("prettier.config.")
        || (name.starts_with("tsconfig") && name.ends_with(".json"))
        || matches!(
            name,
            "hygiene.toml"
                | "rust-toolchain"
                | "rust-toolchain.toml"
                | "Cargo.toml"
                | "Cargo.lock"
                | "build.rs"
                | "rustfmt.toml"
                | ".rustfmt.toml"
                | "clippy.toml"
                | ".clippy.toml"
                | "package.json"
                | "package-lock.json"
                | ".eslintignore"
                | ".npmrc"
                | ".gitignore"
                | ".gitattributes"
        )
}

#[derive(Default)]
pub struct RustGuard {
    tests: BTreeSet<String>,
    suppressions: BTreeMap<String, u64>,
}

impl RustGuard {
    pub fn weakens(&self, before: &Self) -> bool {
        !before.tests.is_subset(&self.tests)
            || self
                .suppressions
                .iter()
                .any(|(rule, count)| *count > before.suppressions.get(rule).copied().unwrap_or(0))
    }
}

pub fn protected_rust(source: &str) -> Result<RustGuard> {
    use quote::ToTokens;
    use syn::visit::Visit;
    #[derive(Default)]
    struct Guard(RustGuard);
    impl<'a> Visit<'a> for Guard {
        fn visit_attribute(&mut self, attr: &'a syn::Attribute) {
            if suppression(&attr.meta) {
                *self
                    .0
                    .suppressions
                    .entry(attr.to_token_stream().to_string())
                    .or_default() += 1;
            }
        }

        fn visit_item_fn(&mut self, item: &'a syn::ItemFn) {
            if item.attrs.iter().any(|attr| {
                attr.path()
                    .segments
                    .last()
                    .is_some_and(|seg| seg.ident == "test")
            }) {
                self.0.tests.insert(item.to_token_stream().to_string());
            }
            syn::visit::visit_item_fn(self, item);
        }

        fn visit_item_mod(&mut self, item: &'a syn::ItemMod) {
            if item.attrs.iter().any(|attr| {
                attr.path().is_ident("cfg")
                    && attr.parse_args::<syn::Ident>().is_ok_and(|id| id == "test")
            }) {
                self.0.tests.insert(item.to_token_stream().to_string());
            }
            syn::visit::visit_item_mod(self, item);
        }
    }
    let mut guard = Guard::default();
    guard.visit_file(&syn::parse_file(source).code("SOURCE_UNPARSEABLE")?);
    Ok(guard.0)
}

fn suppression(meta: &syn::Meta) -> bool {
    if meta.path().is_ident("allow") || meta.path().is_ident("expect") {
        return true;
    }
    if let syn::Meta::List(list) = meta
        && list.path.is_ident("cfg_attr")
    {
        use syn::punctuated::Punctuated;
        return list
            .parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
            .is_ok_and(|items| items.iter().skip(1).any(suppression));
    }
    false
}
