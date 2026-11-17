# Lather icon and README rubric

This rubric samples widely starred Rust developer tools and a CLI library. Stars are a GitHub snapshot from November 17, 2026, and serve as a selection signal.

| Reference | Stars | Practice to carry over |
| --- | ---: | --- |
| [uv](https://github.com/astral-sh/uv) | 92,216 | Start with a one-line value proposition and make installation easy to find. |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | 70,021 | Put concrete commands and the behavior they demonstrate near the top. |
| [bat](https://github.com/sharkdp/bat) | 61,295 | Give the tool a distinctive mark and show what it does in a terminal. |
| [Starship](https://github.com/starship/starship) | 60,774 | Give the mark alt text and accommodate light and dark themes. |
| [Ruff](https://github.com/astral-sh/ruff) | 51,417 | Explain the scope and ground product claims in evidence. |
| [fd](https://github.com/sharkdp/fd) | 45,093 | Use a short demo followed by examples readers can adapt. |
| [just](https://github.com/casey/just) | 36,733 | Connect a short name to a clear workflow and spell out prerequisites. |
| [clap](https://github.com/clap-rs/clap) | 16,884 | Give newcomers a direct starting point and make technical reference easy to find. |

Each row is worth one point when its evidence meets the full pass condition. A missing or inconclusive check earns zero. A release earns 10/10 when all ten checks pass.

| # | Criterion | Pass condition |
| ---: | --- | --- |
| 1 | Distinctive mark | The icon reads as a bar of soap and shows a terminal prompt. It works without a wordmark. |
| 2 | Small-size legibility | Both parts of the terminal prompt and the soap silhouette remain distinguishable at 24 and 32 px against light and dark backgrounds. |
| 3 | Accessible, portable image | The README supplies useful alt text. Its local transparent PNG is square, at most 100 KiB, and present in the crate archive. |
| 4 | Clear first screen | A desktop reader sees the name, the job Lather does, the install command, and the first action. The compact layout stays readable in a narrow viewport. |
| 5 | Install and setup | The README names the crate and executable, identifies the Linux requirement, and gives complete repository registry syntax and both profile prerequisites. |
| 6 | Reproducible first run | Running the documented first command against a registered clean worktree produces the documented JSON fields and four gate outcomes. |
| 7 | Real improvement loop | An observed Rust example shows the resulting decision and score change. The documented commands take the reader from baseline to a verified commit. |
| 8 | Consistent command surface | The README accounts for all seven commands. The CLI and MCP expose matching names, the named config files are used, and the schema flag exposes the cited tool. |
| 9 | Trust and guardrails | The README identifies trusted worktrees, explains code execution and repair versus scoring, and covers all verification exit codes. |
| 10 | Navigation and restraint | GitHub-rendered Markdown has working section links, a license link, and clear progression from setup to workflow and reference. The rendered hero fits both desktop and narrow views. |
