# Lather

Lather measures code hygiene, proposes one small improvement, and checks whether a committed change improves the score. Use the CLI directly or drive the same commands through MCP.

## Install

```sh
cargo install lather-cli --locked
lather --help
```

Run repository checks on Linux with Git and the tools for your project. Register only trusted worktrees. Checks can execute repository code, including build scripts and tests.

| Project | Required tools and configuration | Scored signals |
| --- | --- | --- |
| Rust | Cargo, Clippy, rustfmt, and a committed `Cargo.lock`. Cache Rust dependencies before measuring. | Oversized production files, compiler-reported unused items, and unchecked exits such as `unwrap` and `panic!`. |
| TypeScript or JavaScript | Node 24, a committed `package-lock.json`, root `tsconfig.json`, and local TypeScript, ESLint, and Prettier installed with `npm ci`. JavaScript-only projects need effective `checkJs`. | Oversized production files and effective TypeScript strictness. |

For JavaScript and TypeScript, Lather runs tracked Node test files named `*.test.*`, `*.spec.*`, or `test-*` in the supported extensions. Use separate worktree roots for Rust and JavaScript/TypeScript projects.

## Register a worktree

Create `${XDG_CONFIG_HOME:-$HOME/.config}/lather/repos.toml` and set `root` to the absolute path of a clean Git worktree.

```toml
schema_version = 1

[[repos]]
id = "example"

[[repos.worktrees]]
id = "dev"
root = "/absolute/path/to/example"
```

Set `LATHER_REGISTRY` to use another registry file or `LATHER_HOME` to override the default state directory at `${XDG_STATE_HOME:-$HOME/.local/state}/lather`. Earlier installations can still use the `hygiene-climb` paths and `HYGIENE_CLIMB_*` overrides.

## Lather, rinse, repeat

1. Run `lather scan example --worktree dev` to see the checks and findings.
2. Run `lather baseline example --worktree dev --name initial` to save a baseline. Pass its ID to `lather next example --worktree dev --baseline "$BASELINE_ID"`.
3. Make and commit one change. Set `TIP` to the comparison snapshot returned by `next`, then run:

   ```sh
   lather verify example --worktree dev --baseline "$BASELINE_ID" \
     --against "$TIP" --commit "$(git rev-parse HEAD)"
   ```

Run `next` again after an accepted change. Use `lather score` to compare a stored snapshot, `lather history example` to review decisions, or `lather report` to see trends.

The baseline fixes the scoring denominator for the epoch. Acceptance requires a positive score change with all required checks passing. Checks also guard against losing tests, adding Rust suppressions, and weakening TypeScript compiler settings. A failing baseline opens a repair epoch. Once all gates pass, create a new baseline to begin scoring.

| Verify exit code | Result |
| --- | --- |
| `0` | Accepted |
| `1` | Command error |
| `2` | Rejected |
| `3` | Inconclusive |
| `4` | Repair progress |

Teams can commit a `hygiene.toml` to change the default 500-line file limit and 100-line candidate diff limit:

```toml
schema_version = 1
file_lines = 500
max_diff_lines = 100
```

## Agents

Start `lather --mcp` for the stdio MCP server. Use `lather --llms` for command discovery, `lather verify --schema` for typed arguments and outputs, or `--json` for structured CLI output.

## License

[MIT](LICENSE)
