use crate::model::{Baseline, Decision, Error, Result, StoredSnapshot, Verification};
use crate::repo::valid_id;
use crate::service::{Mode, Proposal, Score, Service, Trend};
use crate::transport::{Reply, command};
use incurs::cli::Cli;
use incurs::mcp::{McpDiscovery, McpServeOptions, McpToolFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, incurs::Args)]
struct RepoArgs {
    /// Repository ID from the washlist.
    repo: String,
}

#[derive(Deserialize, incurs::Args)]
struct SnapshotArgs {
    /// Stored snapshot ID.
    snapshot: i64,
}

#[derive(Deserialize, incurs::Options)]
struct WorktreeOptions {
    /// Registered worktree to measure.
    worktree: String,
}

#[derive(Deserialize, incurs::Options)]
struct BaselineOptions {
    /// Registered worktree to measure.
    worktree: String,
    /// Name of the new scoring or repair epoch.
    name: String,
}

#[derive(Deserialize, incurs::Options)]
struct ScoreOptions {
    /// Baseline ID.
    baseline: i64,
}

#[derive(Deserialize, incurs::Options)]
struct NextOptions {
    /// Registered worktree at the current tip.
    worktree: String,
    /// Baseline ID.
    baseline: i64,
}

#[derive(Deserialize, incurs::Options)]
struct VerifyOptions {
    /// Registered worktree at the candidate commit.
    worktree: String,
    /// Baseline ID.
    baseline: i64,
    /// Exact comparison snapshot ID returned by scrub.
    against: i64,
    /// Full candidate commit SHA.
    commit: String,
}

#[derive(Deserialize, incurs::Options)]
struct PageOptions {
    /// Page size from 1 to 100. Defaults to 20.
    limit: Option<u32>,
    /// Cursor from the previous page.
    before: Option<i64>,
}

impl PageOptions {
    fn bounds(&self) -> Result<(i64, u32)> {
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
struct Page<T> {
    entries: Vec<T>,
    next_cursor: Option<i64>,
}

#[derive(Serialize, JsonSchema)]
struct HistoryEntry {
    id: i64,
    verification: Verification,
}

const NAME: &str = env!("CARGO_BIN_NAME");
const SPOT_CHECK: &str = "spot-check";
const SOAK: &str = "soak";
const SHINE: &str = "shine";
const SCRUB: &str = "scrub";
const RINSE: &str = "rinse";
const WASH_LOG: &str = "wash-log";
const WASH_REPORT: &str = "wash-report";

pub fn app() -> Cli {
    Cli::create(NAME)
        .version(env!("CARGO_PKG_VERSION"))
        .mcp(McpServeOptions {
            tools: McpToolFilter {
                discovery: McpDiscovery::Direct,
                ..Default::default()
            },
            ..Default::default()
        })
        .command(
            SPOT_CHECK,
            command(
                SPOT_CHECK,
                "Run repository checks and measure a committed worktree",
                scan,
            ),
        )
        .command(
            SOAK,
            command(SOAK, "Save a baseline for scoring or gate repair", baseline),
        )
        .command(
            SHINE,
            command(SHINE, "Score a stored snapshot against a baseline", score),
        )
        .command(
            SCRUB,
            command(
                SCRUB,
                "Suggest one small improvement for the current worktree",
                next,
            ),
        )
        .command(
            RINSE,
            command(
                RINSE,
                "Verify and record the result of a committed change",
                verify,
            ),
        )
        .command(
            WASH_LOG,
            command(WASH_LOG, "Read a repository's candidate verdicts", history),
        )
        .command(
            WASH_REPORT,
            command(
                WASH_REPORT,
                "Read baseline trends across measured repositories",
                report,
            ),
        )
}

fn scan(args: RepoArgs, options: WorktreeOptions) -> Result<Reply<StoredSnapshot>> {
    let mut data = Service::open()?.scan(&args.repo, &options.worktree)?;
    data.snapshot.test_inventory.clear();
    data.snapshot.findings.truncate(20);
    if let Some(exits) = &mut data.snapshot.unchecked_exits {
        exits.truncate(20);
    }
    if let Some(items) = &mut data.snapshot.unused_items {
        items.truncate(20);
    }
    let (command, reason) = if data.snapshot.complete() {
        (
            format!(
                "{NAME} {SOAK} {} --worktree {} --name initial",
                args.repo, options.worktree
            ),
            "Freeze this worktree as a scoring or gate-repair baseline",
        )
    } else {
        (
            format!(
                "{NAME} {SPOT_CHECK} {} --worktree {}",
                args.repo, options.worktree
            ),
            "Resolve the unknown checks and run spot-check again",
        )
    };
    Ok(Reply::new(data, command, reason))
}

fn baseline(args: RepoArgs, options: BaselineOptions) -> Result<Reply<Baseline>> {
    let data = Service::open()?.baseline(&args.repo, &options.worktree, &options.name)?;
    let command = format!(
        "{NAME} {SCRUB} {} --worktree {} --baseline {}",
        args.repo, options.worktree, data.id
    );
    Ok(Reply::new(
        data,
        command,
        "Propose one change at this baseline's tip",
    ))
}

fn score(args: SnapshotArgs, options: ScoreOptions) -> Result<Reply<Score>> {
    let data = Service::open()?.score(args.snapshot, options.baseline)?;
    Ok(Reply::new(
        data,
        format!("{NAME} {WASH_REPORT}"),
        "Review baseline trends",
    ))
}

fn next(args: RepoArgs, options: NextOptions) -> Result<Reply<Proposal>> {
    let data = Service::open()?.next(&args.repo, &options.worktree, options.baseline)?;
    let (command, reason) = match data.mode {
        Mode::Scrub | Mode::Repair => (
            format!(
                "{NAME} {RINSE} {} --worktree {} --baseline {} --against {} --commit CANDIDATE_SHA",
                args.repo, options.worktree, options.baseline, data.tip
            ),
            "Apply one change, commit it, and replace CANDIDATE_SHA with its full SHA",
        ),
        Mode::ReadyForBaseline => (
            format!(
                "{NAME} {SOAK} {} --worktree {} --name green",
                args.repo, options.worktree
            ),
            "Freeze a green scoring anchor",
        ),
        Mode::Exhausted => (
            format!("{NAME} {WASH_LOG} {}", args.repo),
            "Review completed improvements",
        ),
    };
    Ok(Reply::new(data, command, reason))
}

fn verify(args: RepoArgs, options: VerifyOptions) -> Result<Reply<Verification>> {
    let mut service = Service::open()?;
    let data = service.verify(
        &args.repo,
        &options.worktree,
        options.baseline,
        options.against,
        &options.commit,
    )?;
    let (command, reason) = match data.decision {
        Decision::Accepted | Decision::RepairProgress => (
            format!(
                "{NAME} {SCRUB} {} --worktree {} --baseline {}",
                args.repo, options.worktree, options.baseline
            ),
            "Continue from this comparison tip",
        ),
        Decision::Rejected | Decision::Inconclusive => (
            format!(
                "{NAME} {SHINE} {} --baseline {}",
                data.after, options.baseline
            ),
            "Inspect the candidate score against this baseline",
        ),
    };
    let exit_code = data.decision.exit_code();
    let mut reply = Reply::new(data, command, reason);
    reply.exit_code = Some(exit_code);
    Ok(reply)
}

fn history(args: RepoArgs, options: PageOptions) -> Result<Reply<Page<HistoryEntry>>> {
    if !valid_id(&args.repo) {
        return Err(Error::new("CONFIG_INVALID", "Invalid repository ID"));
    }
    let (before, limit) = options.bounds()?;
    let mut rows = Service::open()?
        .store
        .history(&args.repo, before, limit + 1)?;
    let next_cursor = page_cursor(&mut rows, limit, |row| row.0);
    let entries = rows
        .into_iter()
        .map(|(id, verification)| HistoryEntry { id, verification })
        .collect();
    Ok(Reply::new(
        Page {
            entries,
            next_cursor,
        },
        format!("{NAME} {WASH_REPORT}"),
        "Review baseline trends",
    ))
}

fn report(_: (), options: PageOptions) -> Result<Reply<Page<Trend>>> {
    let (before, limit) = options.bounds()?;
    let service = Service::open()?;
    let mut ids = service.store.baselines(before, limit + 1)?;
    let next_cursor = page_cursor(&mut ids, limit, |id| *id);
    let entries = ids
        .into_iter()
        .map(|id| service.trend(id))
        .collect::<Result<Vec<_>>>()?;
    let (command, reason) = match (next_cursor, entries.first()) {
        (Some(cursor), _) => (
            format!("{NAME} {WASH_REPORT} --before {cursor} --limit {limit}"),
            "Read the next page",
        ),
        (_, Some(trend)) => (
            format!("{NAME} {WASH_LOG} {}", trend.repo),
            "Review individual candidate verdicts",
        ),
        _ => (
            format!("{NAME} --llms"),
            "Discover commands for measuring a registered worktree",
        ),
    };
    Ok(Reply::new(
        Page {
            entries,
            next_cursor,
        },
        command,
        reason,
    ))
}

fn page_cursor<T>(entries: &mut Vec<T>, limit: u32, id: impl Fn(&T) -> i64) -> Option<i64> {
    let more = entries.len() > limit as usize;
    entries.truncate(limit as usize);
    entries.last().filter(|_| more).map(id)
}

#[cfg(test)]
#[path = "tests/pagination.rs"]
mod tests;
