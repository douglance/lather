<img src="assets/lather.png" alt="Mint bar of soap stamped with a terminal prompt" width="64" height="64" align="right">

# Lather

Measure code hygiene one commit at a time. Lather runs your existing checks, suggests a small improvement, and verifies the commit against a fixed baseline. Use it from the terminal or over MCP.

```sh
cargo install lather-cli --locked
lather --help
```

[Set up a worktree](#set-up-a-worktree) · [Run the wash cycle](#run-the-wash-cycle) · [Connect an MCP client](#mcp)

## Set up a worktree

Use Rust 1.88 or later to install Lather. Run it on Linux with Git. Register only trusted worktrees because checks can execute repository code, including build scripts and tests. Start from a clean, committed worktree.

- **Rust.** Install Cargo, Clippy, and rustfmt. Commit `Cargo.lock` and cache dependencies for offline use. Lather scores oversized files, compiler-reported unused items, and unchecked exits such as `unwrap` and `panic!`.
- **TypeScript or JavaScript.** Use Node 24, a committed `package-lock.json`, and a root `tsconfig.json`. Install TypeScript, ESLint, and Prettier via `npm ci`. JavaScript-only projects need effective `checkJs`. Lather scores oversized files and TypeScript strictness.

Keep Rust and TypeScript/JavaScript in separate worktree roots. The Node test gate runs tracked `*.test.*`, `*.spec.*`, and `test-*` files with supported JavaScript or TypeScript extensions.

Save this as `${XDG_CONFIG_HOME:-$HOME/.config}/lather/washlist.toml` and replace the absolute path:

```toml
schema_version = 1

[[repos]]
id = "example"

[[repos.worktrees]]
id = "dev"
root = "/absolute/path/to/example"
```

## Run the wash cycle

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

**Soak** to save a baseline, then **scrub** to get one proposed improvement. Use the baseline `id` returned by `soak`.

```sh
lather soak example --worktree dev --name initial
lather scrub example --worktree dev --baseline "$BASELINE_ID"
```

**Rinse** after making and committing the change. In this sample, invalid port input should return zero, so the change replaces `value.parse().unwrap()` with `value.parse().unwrap_or_default()`. Set `TIP` to the `tip` snapshot ID returned by `scrub`, then run this from the registered worktree:

```sh
lather rinse example --worktree dev --baseline "$BASELINE_ID" \
  --against "$TIP" --commit "$(git rev-parse HEAD)"
```

The sample commit was accepted. All four gates passed, and the score rose from 80% to 100%.

A scoring baseline fixes the scoring denominator. Lather accepts a commit when the score improves, all four gates pass, and tests and scored signals hold up. A failing baseline starts a repair cycle. Once all gates pass, run `soak` again to begin scoring.

## Command reference

| Command | What it does |
| --- | --- |
| `spot-check` | Run type or compile checks, lint, formatting, and tests. Show findings. |
| `soak` | Save a baseline for scoring or gate repair. |
| `scrub` | Suggest one change for the current baseline tip. |
| `rinse` | Verify and record a committed change. |
| `shine` | Score a stored snapshot against a baseline. |
| `wash-log` | Read the verification history for a repository. |
| `wash-report` | Compare baseline trends across registered worktrees. |

`rinse` returns `0` for accepted, `1` for a command error, `2` for rejected, `3` for inconclusive, and `4` for repair progress. Checks detect removed tests, new Rust suppressions, and weaker TypeScript compiler settings.

## Configuration

Commit a `lather.toml` at the worktree root to override the default limits:

```toml
schema_version = 1
file_lines = 500
max_diff_lines = 100
```

Set `LATHER_WASHLIST` to use a different washlist. Set `LATHER_HOME` to move `wash-log.sqlite3` out of `${XDG_STATE_HOME:-$HOME/.local/state}/lather`. Existing development installs can still read their `hygiene.toml`, `repos.toml`, and `hygiene.sqlite3` files.

## MCP

Start the stdio server with `lather --mcp`. It exposes the seven commands above. Use `lather --llms` for command discovery, `lather rinse --schema` for typed inputs and outputs, and `--json` for structured CLI results.

[MIT license](LICENSE)
