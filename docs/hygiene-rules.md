# Hygiene rules

Run `lather spot-check <repo> --worktree <id> --json` to inspect the measured
signals. The snapshot includes gate diagnostics, scored debt, source locations,
report-only inventory, and unavailable checks. Each signal keeps the total debt
and up to 256 finding locations.

| Signal | Role | Rust | TypeScript and JavaScript |
| --- | --- | --- | --- |
| S1 Compiler | Gate | Cargo workspace and all-target compiler errors | Effective root TypeScript project, including JavaScript with `checkJs` |
| S2 Strictness | Score and guard | Track unconditional `forbid(unsafe_code)` and crate-root or Cargo deny/forbid coverage for `unused_must_use` and `unreachable_code` | Count effective supported strict compiler options, including `isolatedModules` and `verbatimModuleSyntax` from [Matt Pocock's TSConfig cheat sheet](https://www.totaltypescript.com/tsconfig-cheat-sheet), and guard their scope and settings |
| S3 Type escape hatches | Score | Count unsafe expressions, functions, trait declarations and implementations. Unchecked exits and lint exceptions are measured under S24 and S5 | Count explicit `any`, non-null assertions, double assertions through `unknown`, and TypeScript check overrides |
| S4 Lint | Gate | Count Clippy errors and warnings. Compiler-reported dead code contributes to S12 | Run project ESLint and count its diagnostics |
| S5 Suppressions | Score and guard | Count explicit `allow` and `expect` attributes, including those in `cfg_attr`. Guard existing tests and reject new suppressions and coverage exclusions | Count ESLint, Biome, Oxlint and Prettier suppression comments. Guard these, TypeScript directives and recognized coverage exclusions |
| S6 Custom checks | Report | Inventory tracked check paths and configured evidence commands | Same |
| S7 Prose rules | Report | Count imperative bullets in root instruction files, divided by that count plus local check files | Same |
| S8 Dependency boundaries | Gate | Compare detected module references and Cargo workspace dependency declarations to configured source and destination prefixes. Use `package:<name>` for a declared dependency name | Compare resolved import, export and literal load paths to configured prefixes. Use `package:<specifier>` for external or unresolved module specifiers |
| S9 Cycles | Gate and report | Count multi-node cycles among normal and build dependencies. Report source-module cycles and optionally gate them | Count multi-node cycles in the graph of tracked production modules, including type-only edges |
| S10 Design system | Optional score | UI frameworks supply their own semantics. A package boundary can ban a disallowed dependency | Count configured JSX elements, imports and props on bindings directly imported from the design system |
| S11 Exact duplication | Score | Count additional lines in token-identical function bodies of at least 64 tokens. Inventory includes measured function lines | Same for JS/TS functions, methods and other body-bearing function syntax |
| S12 Dead code | Score | Count compiler-reported unused items. Optionally ingest cargo-machete or project evidence | Optionally ingest Knip or project evidence for unused exports, files and dependencies |
| S13 Maintenance markers | Score | Count comment markers and explicit deprecation attributes. The compiler lint gate sees active deprecation warnings | Count comment markers and deprecation tags. Optionally ingest typed ESLint deprecation diagnostics |
| S14 Tests | Gate and guard | Run the test suite and guard existing tests and the ignored-test count | Run discovered tests and guard the existing test inventory and ignored-test count. Optionally guard against new Landmines testing findings |
| S15 Coverage | Optional score and guard | Count uncovered lines and branches in an LLVM or project report. Reject a lower covered fraction | Count uncovered lines and branches in an Istanbul or project report. Reject a lower covered fraction |
| S16 Feedback latency | Report | Record elapsed milliseconds for the gate phase | Same |
| S17 Verification surface | Report | Inventory named hooks, common verification targets, workflow command markers and browser/simulator fixture paths | Also include matching package scripts, including `lint:boundaries` from [setup-ts-deep-modules](https://github.com/mattpocock/skills/blob/main/skills/in-progress/setup-ts-deep-modules/SKILL.md), and Storybook fixtures |
| S18 Module depth | Report | Inventory source code lines and public visibility declarations per file | Inventory source code lines and top-level exports per file |
| S19 Size | Score | Count lines above the configured file or function limit | Same |
| S20 Comments | Report | Inventory code and comment lines and commented-out local declaration candidates | Same for parseable `let`, `const`, and `var` declarations |
| S21 Conventions | Report | Inventory parallel HTTP, date/time, and async-runtime dependencies plus formatter configs | Inventory parallel HTTP, date/time, state, test, runtime-validation, and data-fetching dependencies plus formatter configs |
| S22 Instructions | Report | Inspect root agent instruction size, recognized commands, local link targets and the `CLAUDE.md` pointer | Same |
| S23 Formatting | Gate | Run rustfmt and record drift | Run Prettier and record drift |
| S24 Unchecked failures | Score | Count unchecked production exits, unfinished macros and known placeholder comments | Count empty catch blocks, debugger statements and placeholder comments. Optionally ingest typed ESLint floating-promise diagnostics |
| S25 Accessibility | Score | Use project evidence for a supported UI target | Check for missing `alt` on intrinsic `img` and `area` without a spread. Optionally ingest accessibility lints or project evidence |

S9 findings show a closed dependency path with a reference line for each step.
In the JSON snapshot, `hygiene.inventory` lists the full group of mutually
dependent files and one representative loop per group in `S9.cycle_edges` and
`S9.rust_module_cycle_edges`. Rust module evidence is available even when
`rust_module_cycles` is disabled.

A signal identified by a dotted suffix is a separate measured part of its parent
rule. A configured weight applies to each scored part. Report-only proxies stay
outside the score. A smaller instruction file, greater module depth, or more
custom checks can change without improving the code. Source analysis covers tracked
production files. Rust macro expansion, conditional module selection and indirect
aliases, and JavaScript computed imports and component aliases can require a
project-specific checker. S11 measures exact function bodies, not near-clones.
S20 reports syntax-shaped comments, and cannot distinguish useful explanations
from stale prose. S22 checks that local link paths exist and that familiar commands
are present. The hard gates run the profile commands independently.

## Configure a repository

These optional settings are part of the frozen baseline policy. Start a new
baseline to change the settings or add evidence.

```toml
[hygiene]
function_lines = 80
rust_module_cycles = false

[hygiene.weights]
S19 = 5

[[hygiene.boundaries]]
from = "src/domain"
forbid = "src/ui"

[[hygiene.boundaries]]
from = "src/domain"
forbid = "package:react"

[hygiene.ui]
forbidden_elements = ["button"]
forbidden_imports = ["react-native"]
component_module = "@example/ui"
forbidden_props = ["className", "style"]
```

Path prefixes match whole path segments. Specify both an existing primitive and
its intended design-system replacement in the project's guidance. Catalog policy
changes and manifest edits belong in a new baseline epoch. Lather's verification
loop keeps existing tests, policy inputs and score denominators fixed. Update the
Lather binary and start a fresh epoch before comparing snapshots made with this
catalog. TypeScript S2 flag additions bump `typescript-hygiene-catalog` so
existing baselines stay incomparable until a new epoch starts.

### Evidence commands

Add a trusted command in `[hygiene.evidence]` for the signals your project measures.
Lather runs the command from the worktree. Use an absolute executable outside the
worktree or a tracked executable under `hygiene-checks/`. Keep project-owned check
scripts there and keep the toolchain and coverage workload fixed. A configured
tool failure, malformed output, or out-of-scope finding stops measurement. Tool
changes detected in the executable or direct file arguments make snapshots
incomparable.

```toml
[hygiene.evidence]
S12 = ["hygiene-checks/unused", "lather:knip"]
S13 = ["hygiene-checks/eslint", "lather:eslint"]
S14 = ["hygiene-checks/test-integrity", "lather:landmines"]
S15 = ["hygiene-checks/coverage", "lather:llvm"]
S24 = ["hygiene-checks/eslint", "lather:eslint"]
S25 = ["hygiene-checks/accessibility"]
```

- S12 accepts `lather:knip` or `lather:machete`. [Knip JSON reporter](https://knip.dev/features/reporters) · [cargo-machete JSON](https://github.com/bnjbvr/cargo-machete#json-output)
- S15 accepts `lather:llvm` or `lather:istanbul`. Use full-workload reports and configure branch instrumentation where supported. Coverage counts detect missed instrumented lines and branches. Changed-file weighting is omitted because the patch changes the set of files being weighted. [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) · [Istanbul reports](https://istanbul.js.org/docs/advanced/alternative-reporters/)
- S13, S24 and S25 accept `lather:eslint`. The project must enable the corresponding typed deprecation, floating-promise, or `jsx-a11y` rules. [ESLint JSON](https://eslint.org/docs/latest/use/formatters/#json)
- Without a format selector, S12, S13, S24 and S25 expect `{"findings":[{"path":"src/main.rs","line":42,"detail":"Description"}]}`. S15 expects `{"lines":{"covered":20,"total":24},"branches":{"covered":5,"total":9}}`. Branches are optional when the instrumenter cannot report them.

### Test integrity with Landmines

For TypeScript and JavaScript, pin Oxlint and `@interfere/landmines` in your
project lockfile. Enable the testing plugin and the rules you want checked in
`.oxlintrc.json`:

```json
{
  "jsPlugins": [
    {
      "name": "landmines-testing",
      "specifier": "@interfere/landmines/testing"
    }
  ],
  "rules": {
    "landmines-testing/no-fixed-sleep": "error",
    "landmines-testing/no-unsignaled-poll": "error",
    "landmines-testing/no-flake-masking": "error"
  }
}
```

Commit executable `hygiene-checks/test-integrity` with a lint scope that includes
your tests and their helpers. This example uses the project root:

```sh
#!/bin/sh
exec ./node_modules/.bin/oxlint --format json
```

Start a fresh baseline. Existing reported findings can stay. Verify rejects
increases for any file and rule, even when the same rule improves elsewhere.
`S14.external` shows the findings without affecting the score. Oxlint
configuration and ignore files are protected during verification. Keep the
configured lint scope and installed versions fixed. In the TypeScript and
JavaScript profile, Lather also guards against new Oxlint and ESLint disable
comments in tracked test files. [Landmines testing rules](https://github.com/interfere-inc/landmines#landmines-testing)

See [the advice mapping](advice-coverage.md) for the source advice covered by each
signal, proxy, or manual review step.
