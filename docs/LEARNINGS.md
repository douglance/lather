# Learnings from Matt Pocock's TypeScript thread

Source material: [the 22 Nov 2026 thread](https://x.com/mattpocockuk/status/2124500662436151797), Matt's follow-up, his replies, [the TSConfig cheat sheet](https://www.totaltypescript.com/tsconfig-cheat-sheet), [`setup-ts-deep-modules`](https://github.com/mattpocock/skills/blob/main/skills/in-progress/setup-ts-deep-modules/SKILL.md), and [`codebase-design`](https://github.com/mattpocock/skills/blob/main/skills/engineering/codebase-design/SKILL.md).

Matt asked what should go in `/the-perfect-typescript-codebase` and later said an early unpublished version of that skill already helped a greenfield project. He did not publish that skill. This file records learnings that Lather cannot (or should not) turn into a mechanical check. Checkable items that were missing from the catalog are in S2, S17, and S21; see the same-PR changes and [hygiene-rules.md](hygiene-rules.md).

Labels: **Matt-authored**, **Matt-endorsed**, or **community** (with a typical citation).

## Already measured

These themes showed up often in the thread, and Lather already measures them:

- Compiler and lint as law, not prose — S1, S4, S23. Matt-authored: the skill "describes lint rules, CI etc" ([2124549022585344202](https://x.com/mattpocockuk/status/2124549022585344202), 1 like). Community: [2124537755426267149](https://x.com/salmanr2d2/status/2124537755426267149), [2124521301767487899](https://x.com/TaoMachina/status/2124521301767487899).
- `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`, and `exactOptionalPropertyTypes` — S2. Matt-authored cheat sheet for the first three; community for `exactOptionalPropertyTypes` ([2124521301767487899](https://x.com/TaoMachina/status/2124521301767487899)).
- Ban `any` and other type escape hatches — S3. Community, many low-like replies, e.g. [2124549523556999636](https://x.com/koen_van_geert/status/2124549523556999636), [2124539295146897811](https://x.com/mikewhob/status/2124539295146897811).
- Configured import boundaries and cycles — S8, S9. Matt-authored `setup-ts-deep-modules` and community ([2124518725231788390](https://x.com/Aria_SW_/status/2124518725231788390)).
- Parallel HTTP, date, state, and test-runner libraries — S21 (extended in this change for validation and data fetching).
- Tests must run and existing tests are guarded — S14.

## Not mechanically checkable

Ranked by how often or strongly the theme appeared, then by whether Matt authored or endorsed it.

1. **Enforce package entry-point boundaries (deep modules), not barrel re-exports.** Matt-authored: `setup-ts-deep-modules` and `codebase-design`. Community also asked for seams and no barrels ([2124571128492368280](https://x.com/sixringsbuild/status/2124571128492368280), [2124538756330131727](https://x.com/theomlondhe/status/2124538756330131727)). Lather's S8 can express this when a repo configures prefixes, but it does not infer a `src/packages/<name>/` layout or treat root files as the only public surface. Wiring `dependency-cruiser` remains a project evidence/CI concern; S17 now inventories a `lint:boundaries` script when one exists.

2. **One paved path for errors, forms, and "the shortcuts you refuse," written with the reason next to the rule.** Community, many replies: [2124546527230415158](https://x.com/Hronom/status/2124546527230415158), [2124541845078409689](https://x.com/ArgoLink_/status/2124541845078409689), [2124524905945510252](https://x.com/NaveenBhati1/status/2124524905945510252), [2124519957828411656](https://x.com/Bodok0/status/2124519957828411656). Pattern choice is project-specific. S21 can only inventory duplicate libraries, not a single house error type.

3. **Parse untrusted input at the boundary (often Zod), including model output.** Community: [2124539295146897811](https://x.com/mikewhob/status/2124539295146897811), [2124545796398006281](https://x.com/a_lungeanu/status/2124545796398006281), [2124527010412183683](https://x.com/ersatyamraj/status/2124527010412183683) (1 like), [2124529737531879543](https://x.com/Adojz/status/2124529737531879543). S21 now flags *parallel* validation libraries. It cannot tell whether a handler actually parses at the edge.

4. **Specs outside the repo, ADRs inside, long-form conversation with the agent, voice first.** Matt-endorsed: "Yep sounds super aligned with my approach" ([2124505526323368303](https://x.com/mattpocockuk/status/2124505526323368303), 1 like) in reply to [2124504850650349954](https://x.com/develogue/status/2124504850650349954). Process, not a compiler signal. S22 only inspects instruction-file shape.

5. **Make illegal states unrepresentable; encode invariants and state machines in types.** Community: [2124555212384014653](https://x.com/mojavex1/status/2124555212384014653), [2124511366450983364](https://x.com/Amatewasu/status/2124511366450983364). Domain modeling. Closest catalog note is advice 61 in [advice-coverage.md](advice-coverage.md).

6. **Adopt Effect (or another one true stack) as the default.** Community, relatively high engagement for this thread: [2124535529308127549](https://x.com/gunta85/status/2124535529308127549) (3 likes), plus many zero-like mentions. Matt did not endorse a library choice here. Not added as a required check.

7. **Formal verification / prove classes of bugs cannot exist.** Community: [2124524945409515982](https://x.com/robertpenner/status/2124524945409515982) and related formal-verification replies. Outside the current catalog.

8. **A clean checkout: one documented command yields types, tests, and a working example without local secrets.** Community: [2124588032548577289](https://x.com/sunyilong1/status/2124588032548577289), [2124549029573058899](https://x.com/IndieBizWire/status/2124549029573058899). Needs a trusted, documented command and a secret-free fixture; S17 and S22 only inventory scripts and instruction text.

9. **Tests as living specs, through the public interface, not mocks of internals.** Mix of community ([2124528010607096014](https://x.com/siddhantmohan1/status/2124528010607096014), [2124525671502557431](https://x.com/the_100_9/status/2124525671502557431)) and Matt-authored `setup-ts-deep-modules` / `codebase-design` ("the interface is the test surface"). Lather runs and guards tests (S14) but does not judge oracles or import depth in tests.

10. **Colocated `tests/` vs a single top-level tests folder.** Community disagrees ([2124534475011023006](https://x.com/KaziAhmedDev/status/2124534475011023006), 2 likes, wants a distinct folder; [2124518725231788390](https://x.com/Aria_SW_/status/2124518725231788390) wants colocated tests). Matt's published skill uses per-package `tests/`. No layout mandate was added.

11. **`moduleDetection: "force"`, `erasableSyntaxOnly`, and other non-boolean or lightly cited tsconfig options.** Matt-authored cheat sheet for `moduleDetection: "force"` (string enum, not an S2 boolean). Community for `erasableSyntaxOnly` ([2124511040821952862](https://x.com/Ssisakhti/status/2124511040821952862), 1 like, 1 bookmark). Left out of S2 so `suggest` does not require flags Matt did not put in his default strictness set.

12. **The platonic skill is opinionated and may never be finished.** Matt-authored: [2124507000487006310](https://x.com/mattpocockuk/status/2124507000487006310), [2124530476866249068](https://x.com/mattpocockuk/status/2124530476866249068).
