use crate::model::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, incurs::Options)]
pub(super) struct PageOptions {
    /// Page size from 1 to 100. Defaults to 20.
    limit: Option<u32>,
    /// Cursor from the previous page.
    before: Option<i64>,
}

impl PageOptions {
    pub(super) fn bounds(&self) -> Result<(i64, u32)> {
        let limit = self.limit.unwrap_or(20);
        let before = self.before.unwrap_or(i64::MAX);
        if before < 1 || !(1..=100).contains(&limit) {
            return Err(Error::new(
                "PAGE_INVALID",
                "Use a positive cursor and a limit from 1 to 100",
            ));
        }
        Ok((before, limit))
    }
}

#[derive(Serialize, JsonSchema)]
pub(super) struct Page<T> {
    pub(super) entries: Vec<T>,
    pub(super) next_cursor: Option<i64>,
}

pub(super) fn page_cursor<T>(
    entries: &mut Vec<T>,
    limit: u32,
    id: impl Fn(&T) -> i64,
) -> Option<i64> {
    let more = entries.len() > limit as usize;
    entries.truncate(limit as usize);
    entries.last().filter(|_| more).map(id)
}
