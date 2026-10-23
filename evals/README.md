# Agent evals

Each task is a realistic debugging request, run by a **fresh** headless `omp` session that has
only the `bevy-fairy` MCP server: no repo, no files, no shell. The session is pointed at
`examples/fixture_game.rs`, a headless Bevy game whose state is known from its own `pub const`s, and
its final answer is scored against facts taken from those constants. It measures whether an agent
can answer from the tools alone, and how efficiently.

- `tasks/dev/` — 6 tasks to iterate against.
- `tasks/heldout/` — 3 tasks with different facts, not paraphrases of the dev ones. Run them to
  check that a change generalised rather than fitted the dev set.

## Run

```sh
python3 evals/run.py                               # dev suite, model sonnet
python3 evals/run.py --suite heldout --model haiku
python3 evals/run.py --suite all --task score --task odd_crate
```

Needs a built fixture (`target/debug/examples/fixture_game`, `cargo build --example fixture_game
--features plugin`), a server binary (`~/.cargo/bin/bevy-fairy` by default; `--server-bin` for
another build), and `omp` on `PATH`. The sessions use omp's own login, the one the agent running
the loop already has, so nothing else needs signing in.
Per task the runner starts the fixture on a free port at 60 fps, writes a project MCP config and a
settings overlay into an empty temp dir, runs `omp -p` there with stdin closed, and always kills
the fixture and removes the dir. One JSON line per task is **appended** to `docs/loop/evals.jsonl`
(`--out`); omp's json event stream goes to `evals/transcripts/<date>-<sha>/` (git-ignored).
Stdlib only.

## Isolation

The session runs with `--no-tools --no-skills --no-rules --no-extensions --no-lsp --no-session`, an
overlay that turns autolearn off, and a project MCP config holding the fairy server plus every
server in `~/.omp/agent/mcp.json` switched off by name (a project entry beats the same-named user
entry). Every model request in omp's event stream records the tools it declared
(`requestControls.tools.declared`). The run counts only if every request declared fairy tools
(`mcp__fairy_*`) and nothing else. Otherwise the session is killed, the record has
`"isolation_broken": true`, the runner prints a loud error, stops, and exits 2. A session that
could read files would be measuring something else.

**One known leak:** omp's own base system prompt still reaches the session — its tool-use
conventions and a line describing the workstation. It says nothing about the fixture, so it cannot
supply an answer, but results from a different harness version are not strictly comparable.

A task's `max_turns` is enforced by the runner: the session is killed after that many model
requests, and the record says `"stopped": "max_turns"`.

## Scoring

A task is a TOML file: `id`, `prompt`, `expected_tools` (a hint, never scored), `max_turns`,
`timeout_s`, and `[[facts]]`. Each fact has a `name` and either `regex` (case-insensitive, against
the final answer) or `number_min`/`number_max` (passes if any number in the answer, signed decimals
such as `-7.5` included, lies in the closed range). **A task succeeds only if every fact passes.**
Number facts are the weak spot: an answer that lists many numbers can hit a range by accident, so
keep ranges narrow and pair a number with a regex where the answer has a name in it.

Per run the record holds `tool_calls`, `invalid_calls` (tool results with `isError`), `loops`
(calls repeating an earlier call's name and canonical-JSON arguments, omp's own `i` intent field
left out), `tokens` (the sum of every model request's total), `seconds`, the resolved `model`,
`stop_reason`, and the per-fact booleans. A task succeeds only if the session also ended on its
own (`stop_reason: "stop"`, not killed, not timed out). The closing table gives the success rate,
mean tool calls on solved tasks, and total invalid calls.

A session that spent no tokens never reached a model — a lost login, a network error. Its record
carries `infra_error` with the error text, it is left out of the success rate, and the runner
stops and exits 3.
