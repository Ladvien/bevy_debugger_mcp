# 2026-10-23 tool-reference: brp stops calling a running game down, and the tool reference comes from tools/list

Focus: (none) · Commits: 8d73f17..13da68e (plus this loop-state commit) · Outcome: shipped, released as v0.2.1

## Summary
Two tools told agents something untrue: `brp` said "no game answering" when a watching method
streamed, and `detect_anomaly`'s description named a field it does not have. Both are fixed with
failing-first checks. The change generates `docs/mcp-tools.md` from the server's own `tools/list`,
with a test that fails when it drifts. Released as 0.2.1 under time pressure, so the full gate did
not run on the release commit (see "Release and install").

## Bugs fixed
### B-041: `brp` reported "no game answering" while the game answered
Must-fix: misleads y, blocks n, common/no workaround n — eval transcript `2026-10-23-dcd9805/dev-spinner_stops.jsonl`: `world.get_components+watch` → "no game answering"; reproduced against the fixture (5.1 s timeout).
Fix: `brp` refuses `+watch` methods by name and names the one-shot method; `call_game` reports a reply it cannot read after a success status as an unreadable answer (root cause: every client error mapped to `no_game_text`). · Failing-first checks: `fixture_ground_truth::a_watching_method_through_brp_is_refused_for_what_it_is` (ground-truth), `tool_contract::an_unreadable_answer_is_not_reported_as_no_game`.

### B-039: `detect_anomaly`'s description named `detection_type`, which the tool refuses
Must-fix: misleads y, blocks n, common/no workaround n — `AnomalyRequest` has no such field and denies unknown fields.
Fix: description rewritten from what `detect` does (`src/server/tools/anomaly.rs` handle_detect). · Failing-first check: `tool_list::every_name_a_tool_description_puts_in_backticks_exists`, run against the real binary's `tools/list`; it found this one and no other.

### Reverted: the guide error boxing (71adcb4 → 8d73f17)
It changed the signature of the public `bevy_fairy::server::tools::guide::handle`, a breaking change
for a lint, which would have forced 0.3.0. The crate-wide `result_large_err` sweep is B-052.

## Change: `docs/mcp-tools.md`, generated from `tools/list`
Plan: render every tool's description and argument table from the real binary's `tools/list`; a test fails when the committed file differs · Files: `tests/tool_list.rs`, `docs/mcp-tools.md`, README, CLAUDE.md · Goal: a tool reference that cannot say anything the server does not · Proof: `tool_list::the_tool_reference_is_generated_from_tools_list` failed with no file and passes on the generated 148-line, 13-tool reference.
Candidates considered: the generated reference (priority 3; chosen: no public API change, fits the time); deleting the stress modules with the `--tcp` legacy server (B-038/B-012, priority 2: touches tests and benches across the tree, and deleting files needs the human's OK); a watch tool (B-045, priority 4: larger than the cap).

## Evidence
| Suite | Before (v0.2.0) | After (7b898a1) |
|---|---|---|
| dev success | 6/6 | 6/6 |
| dev mean tool calls, solved | 14.5 | 13.2 |
| dev `isError` results | 4 | 5 |
| held-out | 3/3 (previous iteration, two runs, one disputed pass B-049) | 2/3 on the first run; the failed task (`stuck_angle`) passed 2 of 2 reruns |
| mutation score on touched handlers | — | not run (see below) |

Tests on the fixed code (`/tmp/f4.txt`): `tool_list` 4, `tool_contract` 6, `fixture_ground_truth` 8,
`docs_drift` 8, `instructions` 3 — 29 passed, 0 failed. Failing first (`/tmp/f2.txt`, `/tmp/f3.txt`):
the three new checks failed on the old code for the reasons above.

The held-out failure (`stuck_angle`): the agent queried only entities with a `Transform`, found the
`mover`, and answered "0.0 rad"; the spinner keeps its angle in `fixture_game::Spinner`, not a
`Transform`. It never touched `brp`'s `+watch` path or `detect_anomaly`. The previous iteration's two
held-out runs of the same task passed on both servers.

Live MCP proof: `tools/list` from the fixed binary: `brp`'s description names the `+watch` refusal and
`detect_anomaly`'s no longer names `detection_type`.

## Metrics
| | before (v0.2.0) | after | why |
|---|---|---|---|
| server LOC | 51187 | 51207 | the `+watch` refusal and the unreadable-answer branch |
| plugin LOC | 2604 | 2604 | |
| MCP tools | 13 | 13 | |
| direct dependencies | 53 | 53 | |

## Review
Not run: the human asked to finish and push within 40 minutes. Logged for the next iteration.

## Test and eval diffs
- `tests/tool_list.rs`: `tool_names` now derives from `listed_tools`, which returns the full tool objects; two tests added.
- `tests/tool_contract.rs`: the fake game gains a `fake.unreadable` method that answers with a non-JSON-RPC body; one test added.
- `tests/fixture_ground_truth.rs`: one test added.
- `evals/run.py`: binary paths are resolved before the session's temp dir becomes the cwd. Two
  records in `evals.jsonl` (sha 7b898a1, `isolation_broken: true`, "declared no tools at all") come
  from the unresolved path, before the fix.

## Release and install
Version 0.2.0 → 0.2.1 (patch: no public Rust API changed; the guide signature change was reverted for
that reason) · semver-checks: not run (time) · Channel: local `cargo publish` (trusted publisher not
yet configured, B-043) · Tag: `v0.2.1` on `13da68e`.
crates.io: https://crates.io/crates/bevy_fairy/0.2.1 (published 2026-10-23T20:02:52Z; `cargo publish` built the package before uploading, `Finished dev profile in 7m 31s`) · docs.rs: https://docs.rs/bevy_fairy/0.2.1 (checked next iteration) · Installed binary: `cargo install bevy_fairy --version 0.2.1` from crates.io, started at the time of this commit; verified in the final message. 0.2.0's docs.rs build succeeded (rustdoc page titled "bevy_fairy - Rust").

**Deviations, at the human's direction to finish within 40 minutes:** the ratchet gate on the release
commit was stopped before it finished; `cargo publish` verified the package build; the targeted tests
above ran on the same code; mutation testing, semver-checks and the review did not run.

## Deferred / open
- B-051: the ratchet's per-file clippy attribution (unconfirmed cause).
- B-052: box the crate `Error` (minor release).
- B-049 (human): the disputed held-out task.
- B-043 (human): add the crates.io trusted publisher.
- B-042: `cargo fmt` debt makes CI's fmt step red.
- Mutation tests for this iteration's handlers are scripted in `/tmp/mutants2.sh` but did not run.

## Next focus
Run the full gate, the two mutants and the review that this release skipped, then the watch tool
(B-045): the spinner task still takes 40-48 calls with ~29 repeats.
