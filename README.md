<img src="assets/lather.png" alt="Mint terminal prompt" width="64" height="64" align="right">

# Lather

Lather runs your project's checks, suggests a small improvement, and verifies the commit against a fixed baseline. Use it from the terminal or over MCP.

```sh
cargo install lather-cli --locked
lather --help
```

[Set up a worktree](#set-up-a-worktree) · [Verify a change](#verify-a-change) · [Connect an MCP client](#mcp)

## Set up a worktree

Use Rust 1.88 or later to install Lather. Run it on Linux with Git. Register only trusted worktrees because checks can execute repository code, including build scripts and tests. Start from a clean, committed worktree.

- **Rust.** Install Cargo, Clippy, and rustfmt. Commit `Cargo.lock` and cache dependencies for offline use. Lather scores oversized files, compiler-reported unused items, and unchecked exits such as `unwrap` and `panic!`.
- **TypeScript or JavaScript.** Use Node 24, a committed `package-lock.json`, and a root `tsconfig.json`. Install TypeScript, ESLint, and Prettier via `npm ci`. JavaScript-only projects need effective `checkJs`. Lather scores oversized files and TypeScript strictness.

Keep Rust and TypeScript/JavaScript in separate worktree roots. The Node test gate runs tracked `*.test.*`, `*.spec.*`, and `test-*` files with supported JavaScript or TypeScript extensions.

Save this as `${XDG_CONFIG_HOME:-$HOME/.config}/lather/repositories.toml` and replace the absolute path:

```toml
schema_version = 1

[[repos]]
id = "example"

[[repos.worktrees]]
id = "dev"
root = "/absolute/path/to/example"
```

## Verify a change

**Spot-check** the worktree. This example shows the four gates and an unchecked Rust exit in a sample project:

```sh
lather spot-check example --worktree dev --json
```

Summary of the sample JSON response:

```json
{
  "gates": ["pass", "pass", "pass", "pass"],
  "unchecked_exits": [{"path": "src/lib.rs", "line": 2, "construct": "unwrap"}]
}
```

Save a baseline, then request one improvement. Set `BASELINE_ID` to the `id` returned by `baseline`.

```sh
lather baseline example --worktree dev --name initial
lather suggest example --worktree dev --baseline "$BASELINE_ID"
```

Verify after making and committing the change. In this sample, invalid port input should return zero, so the change replaces `value.parse().unwrap()` with `value.parse().unwrap_or_default()`. Set `TIP` to the `tip` snapshot ID returned by `suggest`, then run this from the registered worktree:

```sh
lather verify example --worktree dev --baseline "$BASELINE_ID" \
  --against "$TIP" --commit "$(git rev-parse HEAD)"
```

Lather 0.2.0 accepted the sample commit. All four gates passed, and the score rose from 80% to 100%.

A scoring baseline fixes the scoring denominator. Lather accepts a commit when the score improves, all four gates pass, and tests and scored signals hold up. A failing baseline starts a repair cycle. Once all gates pass, create a new baseline to begin scoring.

### Rust unsafe-code policy

Build Lather from this checkout and create a fresh Rust baseline to score unsafe-code policy. Lather checks each workspace library and binary crate root for unconditional `#![forbid(unsafe_code)]`, a package Cargo lint at level `forbid`, or the same lint inherited from the workspace. The `rust_unsafe_policy` field in `spot-check --json` maps crate-root paths to the observed policy.

Conditional and module-level attributes leave the crate root uncredited. The measurement covers workspace targets and their declared policies. When Lather cannot read a target or its manifest, the scan reports a metric issue and the snapshot cannot be scored.

## Command reference

| Command | What it does |
| --- | --- |
| `spot-check` | Run type or compile checks, lint, formatting, and tests. Show findings. |
| `baseline` | Save a baseline for scoring or gate repair. |
| `suggest` | Suggest one change for the current baseline tip. |
| `verify` | Verify and record a committed change. |
| `score` | Score a stored snapshot against a baseline. |
| `history` | Read the verification history for a repository. |
| `report` | Compare baseline trends across registered worktrees. |

`verify` returns `0` for accepted, `1` for a command error, `2` for rejected, `3` for inconclusive, and `4` for repair progress. Checks detect removed tests, new Rust suppressions, and weaker TypeScript compiler settings.

## Configuration

Commit a `lather.toml` at the worktree root to override the default limits:

```toml
schema_version = 1
file_lines = 500
max_diff_lines = 100
```

Set `LATHER_REPOS` to use a different repository registry. Set `LATHER_HOME` to store `history.sqlite3` in a different directory. The default is `${XDG_STATE_HOME:-$HOME/.local/state}/lather`.

## MCP

Start the stdio server with `lather --mcp`. It exposes the seven commands above. Use `lather --llms` for command discovery, `lather verify --schema` for typed inputs and outputs, and `--json` for structured CLI results.

If you're upgrading from 0.1.0, your CLI commands, configuration, and history still work. Refresh the tool list in your MCP client and update any tool names you call directly.

[MIT license](LICENSE)
