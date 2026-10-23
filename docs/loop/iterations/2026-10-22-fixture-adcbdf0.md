# 2026-10-22 fixture: the tools are checked against a game whose state is known

Focus: (empty — bootstrapping rule) · Commits: 969e8c7..adcbdf0 · Outcome: shipped (no release — `bevy_fairy` has never been published, and a new name is the human's)

## Summary

The loop had no fixture game, so no tool test compared tool output to ground truth: every one
asserted against a fake BRP endpoint whose answers the test itself wrote. This iteration adds a
headless fixture game with its state in `pub const`s, and five tests that call the server's `brp`
and `status` tools against it. Mutants of the `brp` handler and of the plugin's `status` handler
are each caught. Two bugs were fixed on the way: `bevy-fairy --version` started the server instead
of printing a version, and the loop prompt's evidence file did not exist under the name it used.
The review found that the first fix broke every client config our own installers had written;
that is fixed too.

## Bugs fixed

### B-032: `bevy-fairy --version` started the MCP server; so did any unknown argument
Must-fix: misleads y, blocks y, common/no workaround n — `bevy-fairy --version` exited 1 with
"MCP stdio server error: connection closed: initialized request"; the release check needs it.
Fix: `--version`/`-V` print `bevy-fairy <version>`; any other unknown argument exits 2, naming it
and listing the valid ones (root cause: every unrecognised word fell through to "start stdio
mode") · Failing-first check: `tests/cli_args.rs`, 0/2 before, 2/2 after.

**The review found a regression in this fix, and it is fixed.** Our installers wrote
`"args": ["stdio"]` (`setup-claude.sh`, `install.sh`, the homebrew caveat) and `"args": ["serve"]`
(`scripts/install.sh`, `scripts/setup-claude.sh`, `scripts/quick-setup-macos.sh`). Those worked
only because of the fallthrough, and they are on users' machines. `stdio` and `serve` are now
explicit legacy aliases for `--stdio`, and every installer writes `--stdio` · Check:
`the_legacy_stdio_spellings_installers_wrote_are_still_accepted` — fails with the aliases removed
(exit 2), passes with them.

### B-033: the loop prompt named an evidence file that did not exist
Must-fix: misleads y — `.omp/commands/bevy-debugger-loop.md:9` names
`docs/loop/2026-10-21-bevy-fairy-loop-literature-critique.md`; the previous iteration's rename
sweep changed the reference and not the file. Fix: the file is renamed (it landed in `e72fca7`,
staged when that commit was made) and `LOG.md`'s link follows (`f3d7991`) · Check: `ls` of the
named path, missing before, present after.

## Change: a headless fixture game, and a ground-truth harness

Plan: the bootstrapping change — the fixture plus ground-truth tests for working tools · Files:
`examples/fixture_game.rs`, `tests/fixture_ground_truth.rs`, `Cargo.toml`, `CLAUDE.md` · Goal: a
tool test asserts against values the fixture's own code fixes, never against values the tool or a
hand-written fake produced · Proof: 5 tests, each killed by a mutant of the code it guards.

The fixture is headless (`MinimalPlugins`, `InputPlugin`, `DebuggerPlugin`, BRP over HTTP), so it
runs on this host without a window. Its state: six `Crate`s with fixed weights, one deliberately
negative; a `Score` resource; a `Mover` at a fixed speed; and a `Spinner` with a **planted bug** —
it stops turning after 120 frames while its frame counter keeps climbing. It exposes
`build_app(port, fps)`, so the game a test checks is the one an agent drives:
`cargo run --example fixture_game --features plugin`.

Candidates considered:
- **The fixture and harness** (chosen) — the loop's bootstrapping rule makes it this iteration's
  change, and every later honesty question (B-009, B-010, the eval suite) needs it.
- **The eval suite** — also a bootstrapping item, but its tasks need a fixture to run against;
  it comes next.
- **Delete the dead code** (B-011/B-012/B-030) — `cli.rs` is never compiled and is the source of
  B-031's nonexistent subcommands. Deferred: a deletion, not a bootstrapping prerequisite.

## Evidence

| Suite | Before | After |
|---|---|---|
| dev evals (success / mean calls / invalid calls) | no suite | no suite |
| held-out evals | no suite | no suite |
| ground-truth tests against a fixture | none | 5 |
| mutation: `brp` drops the last array row | — | 3 of 4 tests fail (resource test returns an object, correctly unaffected) |
| mutation: plugin `status` frame frozen at 1 | — | `status_reports_a_frame_count_that_advances` fails |
| mutation: legacy aliases removed | — | `the_legacy_stdio_spellings_..._accepted` fails |
| whole workspace | 523 passed, 4 ignored | 531 passed, 0 failed, 4 ignored |

The gate on the final tree, `adcbdf0`:

```
ratchet: OK (no new errors, warnings or failures)
531 passed; 0 failed; 4 ignored
```

Live, through a real MCP client over stdio (`bevy-fairy --stdio`) against the fixture running as
its own process (`target/debug/examples/fixture_game`, `FIXTURE_PORT` on a free port):

```
server: {'name': 'bevy-fairy', 'version': '0.2.0'}   tools: 14
status:        {"frame":19,"ready":true,...}
status again:  {"frame":49,"ready":true,...}
crates via brp: [-7.5, 1.0, 2.0, 3.0, 4.0, 5.0]          # CRATE_WEIGHTS
score via brp:  {"value":{"points":42}}                  # SCORE_POINTS
spinner via brp: {'angle': 6.000004768371582, 'frames': 201}   # frozen at 120 * 0.05
```

The `brp` mutant (the handler drops the last row of any array result, i.e. "cleans up" the odd
value):

```
test brp_query_returns_exactly_the_crates_the_fixture_spawned ... FAILED
test the_planted_spinner_bug_is_visible_through_the_brp_tool ... FAILED
test the_mover_is_further_along_on_a_later_read ... FAILED
test brp_get_resources_returns_the_fixtures_score ... ok
assertion `left == right` failed: the crates the tool reports must be the crates the fixture spawned
```

The `status` mutant:

```
test status_reports_a_frame_count_that_advances ... FAILED
a running game's frame count must advance between reads:
  {... "frame":1,"ready":true ...} then {... "frame":1,"ready":true ...}
```

A first mutation run of the `brp` handler did not apply — the patched line occurs four times in
`mcp_tools.rs`, the script's guard refused, and without `set -e` the "mutant" run was an
unmutated one (all passed). It was re-run with the mutant pinned to the `brp` handler's line and
the script failing hard if the patch did not land; the results above are from that run.

## Metrics

| | Before | After |
|---|---|---|
| server LOC (`src/server`) | 51212 | 51212 |
| plugin LOC (`src/plugin`) | 2604 | 2604 |
| `src/main.rs` | — | +27 (argument handling) |
| direct dependencies | 53 | 53 |
| public modules | 72 | 72 |
| MCP tools | 14 | 14 |
| clippy warnings, touched files | — | 0 new; `src/main.rs` stays at its baseline 6 `result_large_err` |
| tests | 523 passed, 4 ignored | 531 passed, 4 ignored (+8) |

The test count goes up by 8 (3 CLI, 5 fixture). The example adds 162 lines, which is not counted
in either half.

## Review

An independent reviewer (fresh context, given the diff, the plan and the evidence) reported one
blocking gap, two should-fix and four nits. Skeptic pass, gap by gap:

1. **Blocks — installers' configs now refused.** Survived: no benign reading; existing user
   configs would break. Fixed (`7cf58dd`).
2. **Should-fix — `status` never really checked.** Survived: `frame > 0` holds on every answer.
   Fixed with a two-read advance check, proven by the frozen-frame mutant (`adcbdf0`).
3. **Should-fix — mover speed published, never checked.** Survived. Fixed with a sound upper
   bound; the first draft used a 10% slack that could flake when a frame begins before the first
   read, and was corrected to one maximum frame delta (Bevy's 250 ms).
4. **Nit — port race between tests.** Benign for honesty (fixtures identical, read-only); the
   comment that claimed no race is corrected.
5. **Nit — spinner counter not shown climbing.** Fixed with a second read.
6. **Nit — bad `FIXTURE_FPS` panics obscurely / unparsable values fall back silently.** The
   silent fallback is the defect class this loop removes; fixed.
7. **Nit — `--help`/`--version` version formats differ; README checks version with `--help`.**
   Fixed.

## Test and eval diffs

New: `tests/cli_args.rs` (3), `tests/fixture_ground_truth.rs` (5). Changed:
`tests/docs_drift.rs` — `CHANGELOG.md` is exempt from the stale-name check (`969e8c7`), because
naming what existed before is its job; the check still covers every other doc. No test was
loosened or skipped; `known_red` was not raised.

## Release and install

Version 0.2.0 → 0.2.0 · semver-checks: not run (no published `bevy_fairy` baseline) · Channel:
none · Tag: none.

Not released: `bevy_fairy` has never been published, and publishing a new crate name is the
human's decision (B-029). `.github/workflows/release.yml` is ready for trusted publishing once a
crates.io trusted publisher is configured. Installed binary: rebuilt from `adcbdf0` (Phase 7).

## Deferred / open

- **B-031** — the launchd plist runs `bevy-fairy serve --config …` and the troubleshooting guide
  runs `export-sessions`; the subcommands live only in the never-compiled `src/server/cli.rs`.
  `serve` alone now starts stdio mode (legacy alias); `--config` is refused by name.
- The fixture has no UI button, pickable object or text field: they need a window.
- No eval suite yet; ground-truth tests are the only behavioural gate.
- B-009 (cursor and picking) and B-010 (no input/watch tool) are now testable against the fixture.

## Next focus

The eval suite: 6 dev and 3 held-out tasks against the fixture. The planted spinner bug is the
first task ("the spinner stops turning — why?"), and its answer is pinned by
`the_planted_spinner_bug_is_visible_through_the_brp_tool`.
