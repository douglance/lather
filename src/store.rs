use crate::model::{
    Baseline, Decision, Error, Result, Snapshot, StoredSnapshot, Verification, WithCode,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::path::Path;
use std::time::Duration;

pub const DATABASE_FILES: &[&str] = &["history.sqlite3", "wash-log.sqlite3", "hygiene.sqlite3"];

pub struct Store(Connection);

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).code("STORE_IO")?;
        let mut database = path.join(DATABASE_FILES[0]);
        for filename in DATABASE_FILES {
            let candidate = path.join(filename);
            if candidate.try_exists().code("STORE_IO")? {
                database = candidate;
                break;
            }
        }
        let connection = Connection::open(database).code("STORE_IO")?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .code("STORE_IO")?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS snapshots (
                 id INTEGER PRIMARY KEY, repo TEXT NOT NULL, commit_sha TEXT NOT NULL, payload TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS snapshots_by_repo ON snapshots(repo, id);
             CREATE TABLE IF NOT EXISTS baselines (
                 id INTEGER PRIMARY KEY, name TEXT NOT NULL, repo TEXT NOT NULL,
                 anchor INTEGER NOT NULL REFERENCES snapshots(id), tip INTEGER NOT NULL REFERENCES snapshots(id),
                 repair INTEGER NOT NULL, UNIQUE(repo, name)
             );
             CREATE TABLE IF NOT EXISTS verifications (
                 id INTEGER PRIMARY KEY, repo TEXT NOT NULL, baseline INTEGER NOT NULL REFERENCES baselines(id),
                 payload TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS verifications_by_repo ON verifications(repo, id);",
        ).code("STORE_IO")?;
        Ok(Self(connection))
    }

    pub fn save(&self, snapshot: Snapshot) -> Result<StoredSnapshot> {
        let id = insert_snapshot(&self.0, &snapshot)?;
        Ok(StoredSnapshot { id, snapshot })
    }

    pub fn start(&mut self, name: &str, snapshot: Snapshot) -> Result<Baseline> {
        if !snapshot.complete() || (snapshot.green() && !snapshot.metric_issues.is_empty()) {
            return Err(Error::new(
                "EVIDENCE_INCOMPLETE",
                "Run spot-check to see the checks that could not complete",
            ));
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .code("STORE_IO")?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM baselines WHERE repo=?1 AND name=?2)",
                params![snapshot.repo, name],
                |row| row.get(0),
            )
            .code("STORE_IO")?;
        if exists {
            return Err(Error::new(
                "BASELINE_EXISTS",
                "Choose a new baseline name for this repository",
            ));
        }
        let anchor = insert_snapshot(&tx, &snapshot)?;
        let repair_mode = !snapshot.green();
        tx.execute(
            "INSERT INTO baselines(name,repo,anchor,tip,repair) VALUES (?1,?2,?3,?3,?4)",
            params![name, snapshot.repo, anchor, repair_mode],
        )
        .code("STORE_IO")?;
        let id = tx.last_insert_rowid();
        tx.commit().code("STORE_IO")?;
        Ok(Baseline {
            id,
            name: name.into(),
            anchor,
            tip: anchor,
            repair_mode,
        })
    }

    pub fn baseline(&self, id: i64) -> Result<Baseline> {
        self.0
            .query_row(
                "SELECT id,name,anchor,tip,repair FROM baselines WHERE id=?1",
                [id],
                |row| {
                    Ok(Baseline {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        anchor: row.get(2)?,
                        tip: row.get(3)?,
                        repair_mode: row.get(4)?,
                    })
                },
            )
            .optional()
            .code("STORE_IO")?
            .ok_or_else(|| Error::new("BASELINE_UNKNOWN", "Unknown baseline ID"))
    }

    pub fn snapshot(&self, id: i64) -> Result<StoredSnapshot> {
        let payload: String = self
            .0
            .query_row("SELECT payload FROM snapshots WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .code("STORE_IO")?
            .ok_or_else(|| Error::new("SNAPSHOT_UNKNOWN", "Unknown snapshot ID"))?;
        Ok(StoredSnapshot {
            id,
            snapshot: serde_json::from_str(&payload).code("STORE_CORRUPT")?,
        })
    }

    pub fn record(
        &mut self,
        mut verdict: Verification,
        snapshot: &Snapshot,
    ) -> Result<Verification> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .code("STORE_IO")?;
        verdict.after = insert_snapshot(&tx, snapshot)?;
        if verdict.decision.advances() {
            let updated = tx
                .execute(
                    "UPDATE baselines SET tip=?1 WHERE id=?2 AND tip=?3",
                    params![verdict.after, verdict.baseline, verdict.before],
                )
                .code("STORE_IO")?;
            if updated == 0 {
                verdict.decision = Decision::Inconclusive;
                verdict.reason = "Comparison tip changed while checks were running".into();
            }
        }
        tx.execute(
            "INSERT INTO verifications(repo,baseline,payload) VALUES (?1,?2,?3)",
            params![
                snapshot.repo,
                verdict.baseline,
                serde_json::to_string(&verdict).code("STORE_IO")?
            ],
        )
        .code("STORE_IO")?;
        tx.commit().code("STORE_IO")?;
        Ok(verdict)
    }

    pub fn history(&self, repo: &str, before: i64, limit: u32) -> Result<Vec<(i64, Verification)>> {
        let mut stmt = self.0.prepare(
            "SELECT id,payload FROM verifications WHERE repo=?1 AND id<?2 ORDER BY id DESC LIMIT ?3",
        ).code("STORE_IO")?;
        let rows = stmt
            .query_map(params![repo, before, limit], |row| {
                Ok((row.get(0)?, row.get::<_, String>(1)?))
            })
            .code("STORE_IO")?;
        rows.map(|row| {
            let (id, payload) = row.code("STORE_IO")?;
            Ok((id, serde_json::from_str(&payload).code("STORE_CORRUPT")?))
        })
        .collect()
    }

    pub fn baselines(&self, before: i64, limit: u32) -> Result<Vec<i64>> {
        let mut stmt = self
            .0
            .prepare("SELECT id FROM baselines WHERE id<?1 ORDER BY id DESC LIMIT ?2")
            .code("STORE_IO")?;
        let rows = stmt
            .query_map(params![before, limit], |row| row.get(0))
            .code("STORE_IO")?;
        rows.map(|row| row.code("STORE_IO")).collect()
    }
}

fn insert_snapshot(connection: &Connection, snapshot: &Snapshot) -> Result<i64> {
    connection
        .execute(
            "INSERT INTO snapshots(repo,commit_sha,payload) VALUES (?1,?2,?3)",
            params![
                snapshot.repo,
                snapshot.commit,
                serde_json::to_string(snapshot).code("STORE_IO")?
            ],
        )
        .code("STORE_IO")?;
    Ok(connection.last_insert_rowid())
}
