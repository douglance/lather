use crate::model::{Finding, Result, UncheckedExit, WithCode};
use syn::visit::{self, Visit};

#[derive(Default)]
pub struct RustMetrics {
    pub lines: u64,
    pub oversized: Vec<Finding>,
    pub unchecked_exits: Vec<UncheckedExit>,
    pub issues: Vec<String>,
}

pub fn measure(
    sources: impl Iterator<Item = Result<(String, String)>>,
    limit: u32,
    rust: bool,
) -> Result<RustMetrics> {
    let mut result = RustMetrics::default();
    for source in sources {
        let (path, text) = source?;
        let lines = u32::try_from(text.lines().count()).code("SOURCE_UNPARSEABLE")?;
        result.lines += u64::from(lines);
        if lines > limit {
            result.oversized.push(Finding {
                path: path.clone(),
                lines,
                excess_lines: lines - limit,
            });
        }
        if !rust {
            continue;
        }
        match syn::parse_file(&text) {
            Ok(file) => UncheckedSites {
                path: &path,
                exits: &mut result.unchecked_exits,
            }
            .visit_file(&file),
            Err(error) => result.issues.push(format!("{path}: {error}")),
        }
    }
    result.oversized.sort_by(|lhs, rhs| {
        rhs.excess_lines
            .cmp(&lhs.excess_lines)
            .then_with(|| lhs.path.cmp(&rhs.path))
    });
    result.unchecked_exits.sort_by(|lhs, rhs| {
        (&lhs.path, lhs.line, &lhs.construct).cmp(&(&rhs.path, rhs.line, &rhs.construct))
    });
    Ok(result)
}

struct UncheckedSites<'a> {
    path: &'a str,
    exits: &'a mut Vec<UncheckedExit>,
}

impl UncheckedSites<'_> {
    fn record(&mut self, ident: &syn::Ident) {
        self.exits.push(UncheckedExit {
            path: self.path.into(),
            line: ident.span().start().line,
            construct: ident.to_string(),
        });
    }
}

impl<'ast> Visit<'ast> for UncheckedSites<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !item.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test")
        }) {
            visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !item.attrs.iter().any(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
        }) {
            visit::visit_item_fn(self, item);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "unwrap" || call.method == "expect" {
            self.record(&call.method);
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Some(segment) = mac
            .path
            .segments
            .last()
            .filter(|segment| segment.ident == "panic" || segment.ident == "unreachable")
        {
            self.record(&segment.ident);
        }
    }
}

#[cfg(test)]
#[path = "tests/signals.rs"]
mod tests;
