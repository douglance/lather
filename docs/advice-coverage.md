# Advice coverage

The 90 entries below follow the order of the source advice across its 12 themes.
S1–S25 refer to the [rule catalog](hygiene-rules.md). Where evidence covers only
part of a recommendation, the last column names the judgment or workflow a
snapshot cannot verify. Source thread by [Ryan Lanciaux](https://x.com/ryanlanciaux/status/2122693025952596147).

| # | Advice | Measured evidence | Remaining judgment or workflow |
| --- | --- | --- | --- |
| 01 | Give agents a cohesive codebase | S8, S10, S18, S21 | Cohesion needs domain context. |
| 02 | Put correctness in the codebase | S1, S2, S4, S8, S14 | Tests and rules cover expressed contracts. |
| 03 | Reject bad patterns early | S1, S4, S8, S9, S17 | Editor-time enforcement depends on hook or editor integration. |
| 04 | Express permitted behavior in code | S2, S3, S8, S10, S24 | Domain invariants must be supplied by the project. |
| 05 | Favor compiler feedback and refactoring over prompts | S1–S3, S7 | Impact on agent outcomes needs an experiment. |
| 06 | Put types, boundaries and analysis ahead of prose | S2, S6–S8, S14 | The right allocation of team effort is contextual. |
| 07 | Convert repeated corrections into architecture | S6, S8 | Corrections and whether they recur are absent from the snapshot. |
| 08 | Group related code and expose small interfaces | S1, S8, S14, S18 | Relatedness and interface clarity require domain context. |
| 09 | Apply existing engineering practices to agents | S1–S25 | The relative effect on people and agents needs outcome data. |
| 10 | Use tests, docs, Git, review and fast staging | S14, S16, S17, S22; Git worktree required | Review quality and staging latency need workflow and deployment records. |
| 11 | Tighten compiler, lint and formatting | S1–S5, S23 | Productivity effects need outcome data. |
| 12 | Enforce imports, strict types and accessibility | S1–S5, S8, S20, S25 | The intent and necessity of individual comments need review. |
| 13 | Combine strict checks, approved components and verification | S1–S5, S10, S14, S17 | Example quality and lessons from incidents need review. |
| 14 | Use type and AST rules for unwanted patterns | S2, S3, S5, S6, S10, S24 | Additional unwanted patterns need an explicit checker. |
| 15 | Fail on imports outside the allowed architecture | S8, S10 | Configure the allowed architecture. |
| 16 | Trade shortcut opportunities for strictness | S2, S3, S5, S24 | Developer friction needs observed workflow data. |
| 17 | Make APIs guide callers toward correct use | S1, S2, S18 | Valid calls and hidden domain invariants need the API contract. |
| 18 | Convert instructions into lints | S6, S7 | Matching a sentence’s meaning to an implementation needs review. |
| 19 | Require checks at commit time or in CI | S1, S2, S4, S14, S17 | Hook installation and branch enforcement need live configuration evidence. |
| 20 | Write repository-local lint rules | S4, S6 | Whether a particular rule is valuable needs its intended invariant. |
| 21 | Prefer types, formatters, visual diffs and targeted edits | S1–S4, S17, S23; verification diff budget | Visual diffs need rendered fixtures; edit provenance requires workflow records. |
| 22 | Carry types through generic APIs | S1–S3 | Whether a schema's behavior fulfills the public contract needs tests. |
| 23 | Reject failing lint, missing tests and known placeholders | S4, S14, S24 | Unlisted placeholder phrasings need a project rule or review. |
| 24 | Encode domain knowledge as executable checks | S6, S8, S14 | Domain checks and their semantics must be supplied. |
| 25 | Turn retrospective findings into checks | S6, S7, S17 | Retrospective activity needs process records. |
| 26 | Enforce deprecations | S4, S13 | Age and meaning of plain prose deprecations have no stable source. |
| 27 | Replace recurring patterns or add lint | S4, S6, S11–S13 | Whether different implementations embody the same pattern needs review. |
| 28 | Make reproducible failures deterministic | S6, S14, S17 | The project must supply the reproducer and test oracle. |
| 29 | Check design-system bypasses | S8, S10 | The repository supplies the primitive and component policy. |
| 30 | Track repeated corrections | — | Requires review history and a taxonomy of corrections. |
| 31 | Consolidate duplicate UI and ban platform imports | S8, S10, S11 | Exact clones are measurable. UI equivalence needs rendered and behavioral evidence. |
| 32 | Use consistent language and controls | S10, S21 | Domain vocabulary and semantic equivalence need an explicit glossary or checker. |
| 33 | Allow only approved components | S8, S10 | Configure disallowed imports and intrinsic elements. |
| 34 | Forbid component prop overrides | S10 | Configure the component module and forbidden prop names. |
| 35 | Verify UI with lints, accessibility, performance and screenshots | S10, S17, S25 | Runtime budgets and screenshot expectations need a fixed workload and render oracle. |
| 36 | Keep a local design-system specification | S22 tracks instruction links | Whether a design spec describes the current UI needs review. |
| 37 | Make design systems usable by agents | S8, S10 | Agent usability needs observed tasks. |
| 38 | Let agents verify their work | S1, S4, S14, S17, S23 | Domain correctness still needs test oracles. |
| 39 | Give humans and agents the same verification pass | S1, S4, S14, S17, S23 | Planning and browser or simulator behavior need workflow evidence. |
| 40 | Build deterministic feedback loops | S1, S4, S14, S16, S17 | Browser exploration needs runnable scenarios. |
| 41 | Keep verification feedback intact | Gate diagnostics and finding locations | Whether an agent received full outputs requires delivery telemetry. |
| 42 | Make documentation examples executable | S14, S17 | Worktrees must include the doctest or example runner. |
| 43 | Establish guardrails for safe changes | Six gates; frozen test and policy inputs | Unknown behaviors require additional assertions. |
| 44 | Capture a repeatable UI test | S14, S17 | A fixture’s fidelity to live UI requires execution against an oracle. |
| 45 | Give code automated tests | S14, S15 | Coverage cannot establish correctness. |
| 46 | Cover from units to end-to-end | S14, S15, S17 | Test-first ordering and adversarial review need activity records. |
| 47 | Reproduce a bug with a failing test before a fix | S14, S15; frozen tests | The failing-before evidence comes from a separate run. |
| 48 | Specify behavior in a test and refactor to pass | S14; frozen tests | Establishing that the test captures the intended behavior needs the contract. |
| 49 | Preserve original test and benchmark suites | Frozen test and benchmark paths and existing Rust tests | Reviewer independence and benchmark performance need observed runs. |
| 50 | Verify properties with Lean | S6, S17 can inventory a configured check | The project defines and executes its formal proof obligations. |
| 51 | Delete bad patterns before they are copied | S3, S5, S11–S13, S24 | The actual patterns agents copy need session data. |
| 52 | Treat the codebase as agent memory | S8, S10–S12, S21 | Which patterns an agent learns is behavioral. |
| 53 | Find and remove dead source | S12 | Scheduling is a workflow responsibility. |
| 54 | Remove workarounds, dead code and footguns | S3, S5, S7, S12, S13, S24 | Workarounds without a known pattern need review. |
| 55 | Address debt before it accumulates | Frozen-baseline trend across measured debt | Business and operational debt need other evidence. |
| 56 | Refactor to reduce accumulated debt | S11–S13, S19, S24; verified changes | Code quality beyond the measured catalog needs review. |
| 57 | Audit bugs, performance, debt and missing tests | S1, S4, S11–S16 | Semantic bugs, runtime performance and audit quality need independent evidence. |
| 58 | Fix the underlying problem once | S11 | A common root cause needs behavior and caller context. |
| 59 | Prefer deep modules with small interfaces | S8, S18 | A good abstraction depends on the domain. |
| 60 | Keep code and context navigable | S19, S22 | Descriptive naming and context quality need review. |
| 61 | Encode states and permitted transitions | S1, S2, S14 and repository checks | Discovering an implicit state machine and inferring valid transitions need a specification. |
| 62 | Avoid maintaining two conventions | S21 | Cross-repository divergence needs a multi-repository inventory. |
| 63 | Keep related code together | S8, S9, S18 within the registered worktree | The best repository boundary and agent effects are contextual. |
| 64 | Prioritize program design | S8, S9, S18, S19 | Design quality cannot be inferred from graph shape alone. |
| 65 | Codify architecture with examples and counterexamples | S6, S8–S10, S17 | Example quality needs the intended architecture. |
| 66 | Draw architecture to make it legible | S8, S9 measure the underlying graph | Diagram accuracy and readability need a comparison with the system. |
| 67 | Look for reusable functions before editing | S11 | The search and decisions made during planning need activity records. |
| 68 | Review duplication, efficiency and instruction compliance | S8, S11, S16, S22 | Runtime efficiency and natural-language compliance need specific contracts. |
| 69 | Check if a change belongs elsewhere | S8, S9 | The best home for a behavior needs domain ownership rules. |
| 70 | Avoid duplicate UI implementations | S10, S11 | Lookalike but different bodies need semantic or visual comparison. |
| 71 | Use one agent-instruction entry point | S7, S22 | External agents’ instruction discovery needs runtime evidence. |
| 72 | Keep build and test instructions visible | S1, S14, S22 | Arbitrary shell instructions cannot be safely inferred from prose. |
| 73 | Prevent documentation rot | S22 detects size and broken local link paths | Staleness, generated-doc fidelity and timing need source or workflow evidence. |
| 74 | Point agents to local version-specific docs | S22 checks stable local paths | Compatibility between prose and a version needs an explicit version contract. |
| 75 | Maintain docs and local review guidance | S7, S22 | Completeness for zero-extra-context operation needs a task-based check. |
| 76 | Teach repository-specific patterns | S6–S8, S10, S22 | Whether an agent learned or followed the guidance requires observation. |
| 77 | Remove stale comments | S13, S20 | Most explanatory comments have no machine-readable symbol binding. |
| 78 | Keep only useful comments | S20 | Utility cannot be inferred from density. |
| 79 | Run automated PR review | S17 inventories workflow files | Invocation, findings and review independence need PR event evidence. |
| 80 | Make review repository-specific | S4, S6 | Signal-to-noise needs labeled review outcomes. |
| 81 | Encode local review and use an independent reviewer | S4, S6 | Reviewer identity and bias need authenticated workflow evidence. |
| 82 | Enforce conventions and iterate on findings before human review | S4, S8, S21; verified iteration | Review order and subtle defects require PR evidence. |
| 83 | Avoid repeated context-free review | — | Review round count, context and diminishing returns need PR history. |
| 84 | Move preventable review findings upstream | S1–S5, S8, S10, S14 | Classifying past findings requires review history. |
| 85 | Use a different reviewer from the author | — | Author and reviewer model identity need execution records. |
| 86 | Reconsider what review should solve | — | The desired outcomes and trade-offs are team decisions. |
| 87 | Avoid rushed PRs and debt deferrals | S13, S14, S17 | Urgency, provenance and ownership need workflow records. |
| 88 | Take responsibility and ownership | — | Source does not establish responsibility or accountability. |
| 89 | Improve practices before scaling them with agents | S1–S25 monitor repository checks and debt | Organizational amplification needs longitudinal outcome data. |
| 90 | Review, refactor and understand generated code | S1, S4, S11, S14; verified iteration | Provenance, review and understanding need human or workflow evidence. |
