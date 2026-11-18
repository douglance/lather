mod comments;
mod modules;

use super::{
    Report, Settings,
    source::{Facts, Function},
};
use crate::integrity::suppression;
use crate::model::Result;
use crate::repo::Repo;
use quote::ToTokens;
use std::collections::BTreeMap;
use syn::visit::{self, Visit};

pub fn collect(
    repo: &Repo,
    production: &BTreeMap<String, String>,
    policy: &Settings,
    report: &mut Report,
) -> Result<BTreeMap<String, Facts>> {
    let mut parsed = BTreeMap::new();
    let mut facts = BTreeMap::new();
    for (path, source) in production {
        let Ok(file) = syn::parse_file(source) else {
            report.unavailable.insert(
                "source-analysis".into(),
                format!("Invalid Rust syntax in {path}"),
            );
            continue;
        };
        let mut visitor = Collect::default();
        visitor.visit_file(&file);
        comments::measure(source, &mut visitor.facts);
        facts.insert(path.clone(), visitor.facts);
        parsed.insert(path.as_str(), file);
    }
    modules::measure(repo, &parsed, &mut facts, policy.rust_module_cycles, report)?;
    Ok(facts)
}

#[derive(Default)]
struct Collect {
    facts: Facts,
}

impl Collect {
    fn site(&mut self, id: &str, span: proc_macro2::Span, reason: &str) {
        self.facts
            .findings
            .push((id.into(), span.start().line, reason.into()));
    }

    fn function(&mut self, sig: &syn::Signature, body: &syn::Block) {
        let start = sig.fn_token.span.start().line;
        let end = body.brace_token.span.close().end().line;
        self.facts.functions.push(Function {
            name: sig.ident.to_string(),
            line: start,
            lines: end.saturating_sub(start) as u64 + 1,
            tokens: flatten(body.to_token_stream()),
        });
        if let Some(unsafe_token) = sig.unsafety {
            self.site("S3", unsafe_token.span, "Unsafe function");
        }
    }
}

fn flatten(tokens: proc_macro2::TokenStream) -> Vec<String> {
    let mut result = Vec::new();
    for token in tokens {
        if let proc_macro2::TokenTree::Group(group) = token {
            result.push(format!("{:?}:open", group.delimiter()));
            result.extend(flatten(group.stream()));
            result.push(format!("{:?}:close", group.delimiter()));
        } else {
            result.push(token.to_string());
        }
    }
    result
}

pub(super) fn test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
            || (attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test"))
    })
}

impl<'ast> Visit<'ast> for Collect {
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if suppression(&attr.meta) {
            self.site("S5", attr.pound_token.span, "Lint suppression");
        }
        if attr.path().is_ident("deprecated") {
            self.site("S13", attr.pound_token.span, "Deprecation marker");
        }
    }

    fn visit_visibility(&mut self, vis: &'ast syn::Visibility) {
        if matches!(vis, syn::Visibility::Public(_)) {
            self.facts.exports += 1;
        }
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !test_only(&item.attrs) {
            visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !test_only(&item.attrs) {
            self.function(&item.sig, &item.block);
            visit::visit_item_fn(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !test_only(&item.attrs) {
            self.function(&item.sig, &item.block);
            visit::visit_impl_item_fn(self, item);
        }
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if !test_only(&item.attrs) {
            if let Some(body) = &item.default {
                self.function(&item.sig, body);
            }
            visit::visit_trait_item_fn(self, item);
        }
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        self.site("S3", block.unsafe_token.span, "Unsafe block");
        visit::visit_expr_unsafe(self, block);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if let Some(token) = item.unsafety {
            self.site("S3", token.span, "Unsafe implementation");
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        if let Some(token) = item.unsafety {
            self.site("S3", token.span, "Unsafe trait");
        }
        visit::visit_item_trait(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Some(name) = mac.path.segments.last().map(|p| p.ident.to_string())
            && matches!(name.as_str(), "todo" | "unimplemented" | "dbg")
        {
            self.site("S24", mac.bang_token.span, &format!("{name}! placeholder"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_rust_production_syntax_and_skips_inline_tests() {
        let ast = syn::parse_file(
            r#"
            #[deprecated]
            #[cfg_attr(feature = "x", allow(clippy::all))]
            pub fn old() { todo!(); }
            unsafe fn probe() { unsafe { callback() }; }
            #[cfg(test)] mod tests { fn ignored() { unimplemented!() } }
        "#,
        )
        .unwrap();
        let mut collect = Collect::default();
        collect.visit_file(&ast);
        let ids: Vec<_> = collect
            .facts
            .findings
            .iter()
            .map(|(id, _, _)| id.as_str())
            .collect();
        assert_eq!(ids, ["S13", "S5", "S24", "S3", "S3"]);
        assert_eq!(collect.facts.functions.len(), 2);
        assert_eq!(collect.facts.exports, 1);
    }
}
