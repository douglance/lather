use super::{Projection, Snapshot, normalized};
use std::collections::BTreeMap;

impl Snapshot {
    pub(super) fn compiler_policy_score(
        &self,
        anchor: &Self,
        projection: &Projection,
    ) -> Option<(u32, u32)> {
        let weights = &anchor.policy.hygiene;
        let mut score = 0;
        let mut weight = 0;
        match (&self.strict_flags, &anchor.strict_flags) {
            (None, None) => {}
            (Some(flags), Some(initial)) => {
                let total = crate::strictness::FLAGS.len();
                let debt =
                    total.saturating_sub(flags.len().saturating_add(projection.more_strict_flags));
                let denominator = total.saturating_sub(initial.len()).max(1);
                let part = weights.weight("S2", 20);
                weight += part;
                score += part * normalized(debt as u128, denominator as u128);
            }
            _ => return None,
        }
        match (&self.rust_unsafe_policy, &anchor.rust_unsafe_policy) {
            (Some(roots), Some(initial)) => {
                if !roots.is_empty() {
                    let debt = unprotected_roots(roots)
                        .saturating_sub(projection.fewer_unprotected_rust_roots);
                    let denominator = unprotected_roots(initial).max(1);
                    let part = weights.weight("S2", 20);
                    weight += part;
                    score += part * normalized(debt as u128, denominator as u128);
                }
            }
            (None, None) => {}
            _ => return None,
        }
        Some((score, weight))
    }
}

fn unprotected_roots(roots: &BTreeMap<String, bool>) -> usize {
    roots.values().filter(|enforced| !**enforced).count()
}
