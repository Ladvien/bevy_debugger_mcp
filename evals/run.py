#!/usr/bin/env python3
"""Agent eval runner for bevy_fairy. Stdlib only. See evals/README.md."""

import argparse
import datetime
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TASK_DIR = REPO / "evals" / "tasks"
SERVER_NAME = "fairy"
TOOL_PREFIX = f"mcp__{SERVER_NAME}_"
FIXTURE_FPS = "60"

# The session gets the fairy server and nothing else: no built-in tools (--no-tools), no skills,
# rules, extensions, LSP or session file, and autolearn's tools switched off by the overlay below.
# The user's own MCP servers are switched off by name in the session's project config. The
# isolation check reads the tools each model request actually declared, so it decides whether
# all of that held.
OMP_FLAGS = ["--no-tools", "--no-session", "--no-skills", "--no-rules", "--no-extensions",
             "--no-lsp", "--no-title", "--mode", "json"]
OVERLAY = "autolearn:\n  enabled: false\nskills:\n  enabled: false\n"
USER_MCP = Path.home() / ".omp" / "agent" / "mcp.json"


# --------------------------------------------------------------------------- tasks / scoring


def load_tasks(suite):
    suites = ["dev", "heldout"] if suite == "all" else [suite]
    tasks = []
    for s in suites:
        for path in sorted((TASK_DIR / s).glob("*.toml")):
            with open(path, "rb") as f:
                task = tomllib.load(f)
            validate_task(task, path)
            task["suite"] = s
            tasks.append(task)
    return tasks


def validate_task(task, path):
    for key, typ in (("id", str), ("prompt", str), ("expected_tools", list),
                     ("max_turns", int), ("timeout_s", int), ("facts", list)):
        if not isinstance(task.get(key), typ):
            raise SystemExit(f"{path}: missing or mistyped `{key}`")
    if not task["facts"]:
        raise SystemExit(f"{path}: a task needs at least one fact")
    for fact in task["facts"]:
        has_re = "regex" in fact
        has_num = "number_min" in fact or "number_max" in fact
        if "name" not in fact or has_re == has_num:
            raise SystemExit(f"{path}: each fact needs `name` and exactly one of regex / number range")
        if has_num and not ("number_min" in fact and "number_max" in fact):
            raise SystemExit(f"{path}: fact {fact['name']!r} needs both number_min and number_max")
        if has_re:
            re.compile(fact["regex"])


_NUMBER = re.compile(r"(?<![\w.])\d+(?:\.\d+)?")


def numbers_in(text):
    """Every number in `text`, signed. A '-' or U+2212 directly before a number is a sign only when
    what precedes the sign is not part of a word or number ("5-7.5" is a subtraction, "(-7.5)" and
    "weight: -7.5" are negative numbers)."""
    out = []
    for m in _NUMBER.finditer(text):
        value = float(m.group())
        start = m.start()
        if start >= 1 and text[start - 1] in "-\u2212":
            before = text[start - 2] if start >= 2 else " "
            if not (before.isalnum() or before in "._)"):
                value = -value
        out.append(value)
    return out


def score_fact(fact, answer):
    if "regex" in fact:
        return re.search(fact["regex"], answer, re.IGNORECASE) is not None
    lo, hi = fact["number_min"], fact["number_max"]
    return any(lo <= n <= hi for n in numbers_in(answer))


def score_answer(task, answer):
    return {fact["name"]: score_fact(fact, answer) for fact in task["facts"]}


# --------------------------------------------------------------------------- stream parsing


def declared_problems(message):
    """Problems with the tools one model request declared; empty means isolated."""
    declared = ((message.get("requestControls") or {}).get("tools") or {}).get("declared")
    if declared is None:
        return ["a model request carried no declared-tools list, so isolation cannot be verified"]
    foreign = [t for t in declared if not str(t).startswith(TOOL_PREFIX)]
    if foreign:
        return [f"tools outside {TOOL_PREFIX}*: {foreign}"]
    if not declared:
        return ["the session declared no tools at all: the fairy server did not connect"]
    return []


def check_isolation(events):
    """Every model request in the session must have declared only fairy tools."""
    messages = assistant_messages(events)
    if not messages:
        return ["no model request was seen, so isolation could not be verified"]
    problems = []
    for message in messages:
        for problem in declared_problems(message):
            if problem not in problems:
                problems.append(problem)
    return problems


def stream_metrics(events):
    """tool_calls, invalid_calls, loops and the final assistant message, from omp json events."""
    tool_calls = invalid = loops = 0
    seen = set()
    for ev in events:
        message = ev.get("message") or {}
        if ev.get("type") != "message_end":
            continue
        if message.get("role") == "assistant":
            for block in message.get("content") or []:
                if block.get("type") == "toolCall":
                    tool_calls += 1
                    # omp adds an `i` (intent) argument of its own; it is not part of the call.
                    args = {k: v for k, v in (block.get("arguments") or {}).items() if k != "i"}
                    key = (block.get("name"), json.dumps(args, sort_keys=True, separators=(",", ":")))
                    if key in seen:
                        loops += 1
                    seen.add(key)
        elif message.get("role") == "toolResult" and message.get("isError") is True:
            invalid += 1
    messages = assistant_messages(events)
    return tool_calls, invalid, loops, (messages[-1] if messages else None)


def answer_text(message):
    return "\n".join(b.get("text", "") for b in (message or {}).get("content") or []
                     if b.get("type") == "text").strip()


def assistant_messages(events):
    return [e["message"] for e in events
            if e.get("type") == "message_end" and (e.get("message") or {}).get("role") == "assistant"]


def usage_tokens(events):
    """Tokens processed across the session: the sum of every model request's total."""
    return sum(int(((m.get("usage") or {}).get("totalTokens")) or 0) for m in assistant_messages(events))


def parse_events(path):
    events = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return events


# --------------------------------------------------------------------------- processes


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def kill_group(proc):
    if proc is None or proc.poll() is not None:
        # The group can outlive its leader (an MCP server child), so signal it regardless.
        if proc is not None:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        return
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        proc.kill()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass


def start_fixture(fixture_bin, port, log_path):
    env = dict(os.environ, FIXTURE_PORT=str(port), FIXTURE_FPS=FIXTURE_FPS)
    log = open(log_path, "wb")
    proc = subprocess.Popen([str(fixture_bin)], env=env, stdin=subprocess.DEVNULL, stdout=log,
                            stderr=subprocess.STDOUT, start_new_session=True)
    log.close()
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"fixture exited early with code {proc.returncode}; see {log_path}")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.5):
                return proc
        except OSError:
            time.sleep(0.1)
    kill_group(proc)
    raise RuntimeError(f"fixture did not accept TCP on port {port} within 60s")


def write_session_config(directory, server_bin, port):
    """The session's project config: the fairy server, and every user-level MCP server switched
    off by name. omp reads a project entry before the same-named user entry, so `enabled: false`
    here wins. Returns the settings overlay's path."""
    servers = {SERVER_NAME: {
        "command": str(server_bin), "args": ["--stdio"],
        "env": {"BEVY_BRP_HOST": "127.0.0.1", "BEVY_BRP_PORT": str(port)}}}
    if USER_MCP.is_file():
        for name in json.loads(USER_MCP.read_text()).get("mcpServers") or {}:
            if name != SERVER_NAME:
                servers[name] = {"command": "false", "enabled": False}
    omp_dir = Path(directory) / ".omp"
    omp_dir.mkdir()
    (omp_dir / "mcp.json").write_text(json.dumps({"mcpServers": servers}))
    overlay = Path(directory) / "overlay.yml"
    overlay.write_text(OVERLAY)
    return overlay


def run_agent(task, model, overlay, cwd, transcript):
    """Run one headless omp session. Returns (timed_out, stopped_why, stderr, exit_code). Tees
    the event stream into `transcript`, and kills the session the moment a model request declares
    a foreign tool or the task's turn limit is passed."""
    cmd = ["omp", "-p", task["prompt"], *OMP_FLAGS, "--config", str(overlay), "--model", model,
           "--max-time", str(task["timeout_s"])]
    proc = subprocess.Popen(cmd, cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, start_new_session=True)
    state = {"turns": 0, "stopped": None}
    stderr_chunks = []

    def pump_stdout():
        with open(transcript, "wb") as out:
            for raw in proc.stdout:
                out.write(raw)
                out.flush()
                try:
                    ev = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                message = ev.get("message") or {}
                if ev.get("type") != "message_end" or message.get("role") != "assistant":
                    continue
                state["turns"] += 1
                if declared_problems(message):
                    state["stopped"] = "isolation"
                    kill_group(proc)
                elif state["turns"] > task["max_turns"]:
                    state["stopped"] = "max_turns"
                    kill_group(proc)

    def pump_stderr():
        stderr_chunks.append(proc.stderr.read())

    threads = [threading.Thread(target=pump_stdout, daemon=True),
               threading.Thread(target=pump_stderr, daemon=True)]
    for t in threads:
        t.start()
    started = time.monotonic()
    timed_out = False
    try:
        # --max-time asks omp to stop; the margin covers its shutdown.
        proc.wait(timeout=task["timeout_s"] + 30)
    except subprocess.TimeoutExpired:
        timed_out = True
    finally:
        kill_group(proc)
        for t in threads:
            t.join(timeout=10)
    # omp stopping itself at --max-time is a timeout too, whatever its last message says.
    timed_out = timed_out or time.monotonic() - started >= task["timeout_s"]
    err = b"".join(stderr_chunks).decode("utf-8", "replace").strip()
    if err:
        Path(str(transcript) + ".stderr").write_text(err)
    return timed_out, state["stopped"], err, proc.returncode


# --------------------------------------------------------------------------- one task


def run_task(task, args, transcripts_dir, date, sha):
    started = time.monotonic()
    transcript = transcripts_dir / f"{task['suite']}-{task['id']}.jsonl"
    record = {
        "date": date, "sha": sha, "suite": task["suite"], "task": task["id"], "success": False,
        "tool_calls": 0, "invalid_calls": 0, "loops": 0, "tokens": 0, "seconds": 0.0,
        "model": None, "facts": {f["name"]: False for f in task["facts"]},
        "isolation_broken": False, "server_bin": str(args.server_bin),
        "transcript": str(transcript),
    }
    fixture = None
    tmp = tempfile.mkdtemp(prefix="fairy-eval-")
    try:
        port = free_port()
        fixture = start_fixture(args.fixture_bin, port,
                                transcripts_dir / f"{task['suite']}-{task['id']}.fixture.log")
        overlay = write_session_config(tmp, args.server_bin, port)
        timed_out, stopped, err, exit_code = run_agent(task, args.model, overlay, tmp, transcript)
    finally:
        kill_group(fixture)
        shutil.rmtree(tmp, ignore_errors=True)

    record["seconds"] = round(time.monotonic() - started, 1)
    events = parse_events(transcript) if transcript.exists() else []
    tool_calls, invalid, loops, final = stream_metrics(events)
    answer = answer_text(final)
    # Held-out answers stay out of the record: evals.jsonl is read while working on changes, and
    # the held-out set exists to be answered by agents that never saw it. The transcript keeps it.
    record_answer = answer if task["suite"] != "heldout" else None
    record.update(tool_calls=tool_calls, invalid_calls=invalid, loops=loops,
                  tokens=usage_tokens(events), answer=record_answer,
                  num_turns=len(assistant_messages(events)),
                  stop_reason=(final or {}).get("stopReason"),
                  timed_out=timed_out, stopped=stopped, exit_code=exit_code)
    if final is not None:
        record["model"] = final.get("model")

    # A session that spent no tokens never reached a model (an expired login, a network error).
    # Counting it as the agent failing the task would put an outage into the success rate.
    if record["tokens"] == 0:
        record["infra_error"] = answer or err[-500:] or "the session produced no model request"
        return record

    problems = check_isolation(events)
    if problems:
        record["isolation_broken"] = True
        record["isolation_problems"] = problems
        print("\n" + "!" * 78, file=sys.stderr)
        print(f"ISOLATION BROKEN on task {task['id']}: " + "; ".join(problems), file=sys.stderr)
        print("The session could reach more than the fairy MCP server. This run does not count.",
              file=sys.stderr)
        print("!" * 78 + "\n", file=sys.stderr)
        return record

    record["facts"] = score_answer(task, answer)
    record["success"] = record["stop_reason"] == "stop" and stopped is None and not timed_out \
        and all(record["facts"].values())
    return record


# --------------------------------------------------------------------------- main


def git_sha():
    try:
        return subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=REPO, check=True,
                              capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def summarize(records):
    print()
    print(f"{'suite':8} {'task':32} {'ok':4} {'calls':>5} {'bad':>4} {'loop':>4} {'tokens':>9} {'secs':>6}")
    for r in records:
        ok = "ISO!" if r["isolation_broken"] else (
            "INFR" if r.get("infra_error") else ("pass" if r["success"] else "FAIL"))
        print(f"{r['suite']:8} {r['task']:32} {ok:4} {r['tool_calls']:>5} {r['invalid_calls']:>4} "
              f"{r['loops']:>4} {r['tokens']:>9} {r['seconds']:>6}")
        if not r["success"] and not r["isolation_broken"] and not r.get("infra_error"):
            missed = [n for n, ok_ in r["facts"].items() if not ok_]
            print(f"{'':8} missed facts: {', '.join(missed) or '(none; no result message)'}")
    counted = [r for r in records if not r["isolation_broken"] and not r.get("infra_error")]
    solved = [r for r in counted if r["success"]]
    rate = len(solved) / len(counted) if counted else 0.0
    mean_calls = sum(r["tool_calls"] for r in solved) / len(solved) if solved else float("nan")
    print(f"\nsuccess rate: {len(solved)}/{len(counted)} = {rate:.0%}   "
          f"mean tool calls on solved tasks: {mean_calls:.1f}   "
          f"total invalid calls: {sum(r['invalid_calls'] for r in counted)}")


def main(argv=None):
    ap = argparse.ArgumentParser(description="Run the bevy_fairy agent evals.")
    ap.add_argument("--suite", choices=["dev", "heldout", "all"], default="dev")
    ap.add_argument("--task", action="append", default=[], help="task id; repeatable")
    ap.add_argument("--model", default="sonnet")
    ap.add_argument("--fixture-bin", type=Path, default=REPO / "target/debug/examples/fixture_game")
    ap.add_argument("--server-bin", type=Path, default=Path.home() / ".cargo/bin/bevy-fairy")
    ap.add_argument("--out", type=Path, default=REPO / "docs/loop/evals.jsonl",
                    help="JSONL file; appended to")
    ap.add_argument("--sha", default=None, help="default: git rev-parse --short HEAD")
    ap.add_argument("--transcripts", type=Path, default=None,
                    help="default: evals/transcripts/<date>-<sha>/")
    args = ap.parse_args(argv)
    # The session runs in an empty temp dir, so a relative --server-bin would not resolve there.
    args.server_bin = args.server_bin.resolve()
    args.fixture_bin = args.fixture_bin.resolve()

    sha = args.sha or git_sha()
    date = datetime.date.today().isoformat()
    transcripts_dir = args.transcripts or REPO / "evals" / "transcripts" / f"{date}-{sha}"

    tasks = load_tasks(args.suite)
    if args.task:
        known = {t["id"] for t in tasks}
        unknown = [t for t in args.task if t not in known]
        if unknown:
            raise SystemExit(f"unknown task id(s) for suite {args.suite!r}: {unknown}; known: {sorted(known)}")
        tasks = [t for t in tasks if t["id"] in args.task]
    for label, path in (("fixture", args.fixture_bin), ("server", args.server_bin)):
        if not path.is_file():
            raise SystemExit(f"{label} binary not found: {path}")
    if shutil.which("omp") is None:
        raise SystemExit("`omp` is not on PATH")

    transcripts_dir.mkdir(parents=True, exist_ok=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)

    records = []
    for task in tasks:
        print(f"[{task['suite']}] {task['id']} ...", flush=True)
        record = run_task(task, args, transcripts_dir, date, sha)
        records.append(record)
        with open(args.out, "a", encoding="utf-8") as f:
            f.write(json.dumps(record) + "\n")
        if record["isolation_broken"]:
            print("Refusing to run further tasks: isolation is broken.", file=sys.stderr)
            break
        if record.get("infra_error"):
            print(f"Stopping: the session never reached a model: {record['infra_error']}",
                  file=sys.stderr)
            break

    summarize(records)
    if any(r["isolation_broken"] for r in records):
        return 2
    return 3 if any(r.get("infra_error") for r in records) else 0


if __name__ == "__main__":
    sys.exit(main())
