"""Prompt-cache and output-limit replay: one coding agent's session, sent as an OpenAI-format client sends it, through
each gateway to a Claude model on the fake provider's Anthropic API, which bills it as Anthropic's prompt cache would.

    python -I benchmark/resilience/cache_replay.py --out benchmark/results/2026-11-15-cache-replay

The session is synthetic and deterministic: a 16,000-character system prompt, six tools, a task, then 15 tool calls,
each answered with about 4,000 characters of file content; turn k sends the whole conversation so far, with the tools,
streamed, without `max_tokens` (as Codex and OpenCode send it). What reaches the "provider" is recorded: the cache
markers, `max_tokens`, and the usage the simulated cache returns (new input, cache read, cache write). Costs are at the
Claude Haiku 5.5 list prices recorded in benchmark/reports/2026-11-14-codex-claude-prompt-cache.md.

Arms: `pygg` (as shipped), `pygg-cache-off` (`PYGG_ANTHROPIC_AUTO_CACHE=off`, a plain translating gateway),
`litellm` (LiteLLM proxy, defaults), `litellm-inject` (LiteLLM with `cache_control_injection_points` on the system
message and the last message, its documented prompt-caching setup). Standard library only.
"""

import argparse
import http.client
import json
import os
import random
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
FAKE = "http://127.0.0.1:9910"  # a second fake (container pyggbench-fake2), so this can run beside run.py
PRICE = {"input": 0.10, "output": 0.50, "cache_read": 0.01, "cache_write": 0.125}  # USD per million tokens
WORDS = ("const let function return if else for while await async import export from value result error index item "
         "list map set key data config parse token cache request response client server router model limit retry "
         "timeout stream chunk buffer state store record ledger cost price usage session attempt fallback").split()


def text(rng, chars):
    out, n = [], 0
    while n < chars:
        w = rng.choice(WORDS)
        out.append(w)
        n += len(w) + 1
    return " ".join(out)[:chars]


def session(turns=15):
    """The requests an agent sends, turn by turn: OpenAI Chat Completions bodies."""
    rng = random.Random(20261115)
    system = "You are a coding agent working in a Git repository. " + text(rng, 16_000)
    tools = []
    for name in ("read_file", "list_dir", "search", "apply_patch", "run_command", "write_file"):
        tools.append({"type": "function", "function": {"name": name, "description": text(rng, 600),
                      "parameters": {"type": "object", "properties": {"path": {"type": "string"},
                                                                      "content": {"type": "string"}},
                                     "required": ["path"]}}})
    messages = [{"role": "system", "content": system},
                {"role": "user", "content": "Fix the failing test in src/router.js. " + text(rng, 800)}]
    requests = []
    for k in range(turns):
        requests.append({"model": None, "messages": json.loads(json.dumps(messages)), "tools": tools, "stream": True,
                         "stream_options": {"include_usage": True}})
        call = {"id": f"call_{k}", "type": "function",
                "function": {"name": rng.choice(["read_file", "search", "apply_patch"]),
                             "arguments": json.dumps({"path": f"src/file{k}.js"})}}
        messages.append({"role": "assistant", "content": None, "tool_calls": [call]})
        messages.append({"role": "tool", "tool_call_id": f"call_{k}", "content": text(rng, 4_000)})
    return requests


def http_json(method, url, body=None, timeout=10):
    req = urllib.request.Request(url, method=method, data=json.dumps(body).encode() if body is not None else None,
                                 headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode()


def wait_port(port, seconds=60):
    deadline = time.time() + seconds
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return True
        except OSError:
            time.sleep(0.3)
    return False


def to_responses(body):
    """The same turn as Codex sends it: OpenAI's Responses API."""
    items, instructions = [], None
    for m in body["messages"]:
        if m["role"] == "system":
            instructions = m["content"]
        elif m["role"] == "user":
            items.append({"type": "message", "role": "user", "content": [{"type": "input_text", "text": m["content"]}]})
        elif m["role"] == "assistant":
            for c in m.get("tool_calls") or []:
                items.append({"type": "function_call", "call_id": c["id"], "name": c["function"]["name"],
                              "arguments": c["function"]["arguments"]})
        elif m["role"] == "tool":
            items.append({"type": "function_call_output", "call_id": m["tool_call_id"], "output": m["content"]})
    tools = [{"type": "function", **t["function"]} for t in body.get("tools") or []]
    out = {"model": body["model"], "instructions": instructions, "input": items, "stream": body.get("stream", False)}
    if tools:
        out["tools"] = tools
    return out


API = {"path": "/v1/chat/completions"}


def send(port, key, body, extra_headers=None):
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=60)
    if API["path"] == "/v1/responses":
        body = to_responses(body)
    conn.request("POST", API["path"], json.dumps(body),
                 {"content-type": "application/json", "authorization": f"Bearer {key}", **(extra_headers or {})})
    resp = conn.getresponse()
    raw = resp.read().decode(errors="replace")
    conn.close()
    return resp.status, raw


class PyggArm:
    port = 8791

    def __init__(self, args, name, cache_off):
        self.args, self.name, self.cache_off = args, name, cache_off
        self.work = Path(args.work) / "replay" / name
        self.path = "q" if cache_off else "p"

    def cli(self, db, *argv, env=None):
        out = subprocess.run([self.args.pygg_exe, "--db", str(db), *argv], capture_output=True, text=True,
                             env={**os.environ, **(env or {})}, timeout=120)
        if out.returncode != 0:
            raise RuntimeError(f"gateway {argv[0]}: {out.stderr.strip()[:300]}")
        return out.stdout

    def start(self):
        if self.work.exists():
            shutil.rmtree(self.work)
        self.work.mkdir(parents=True)
        db = self.work / "store.db"
        out = self.cli(db, "add-connection", "--preset", "anthropic", "--label", "Fake Anthropic", "--endpoint",
                       f"http://127.0.0.1:9910/{self.path}/v1", "--key-env", "FAKE_ANTHROPIC_KEY",
                       env={"FAKE_ANTHROPIC_KEY": "fake-anthropic-key"})
        self.provider = out.split("Added connection ", 1)[1].split(" ", 1)[0]
        self.cli(db, "model", "--model", f"{self.provider}/claude-haiku-5-5", "--tier", "T2", "--context", "200000",
                 "--max-output", "64000", "--tools", "yes", "--price-in", "0.10", "--price-out", "0.50",
                 "--price-cached", "0.01", "--price-cache-write", "0.125", "--price-source", "benchmark")
        self.token = self.cli(db, "create-client", "--name", "replay").strip().splitlines()[-1].strip()
        env = {**os.environ}
        if self.cache_off:
            env["PYGG_ANTHROPIC_AUTO_CACHE"] = "off"
        self.log = open(self.work / "gateway.log", "w")
        self.proc = subprocess.Popen([self.args.pygg_exe, "--db", str(db), "serve", "--port", str(self.port)],
                                     stdout=self.log, stderr=subprocess.STDOUT, env=env)
        if not wait_port(self.port):
            raise RuntimeError("pygg did not start")
        self.db = db

    def model(self):
        return f"{self.provider}/claude-haiku-5-5"

    def key(self):
        return self.token

    def stop(self):
        self.proc.terminate()
        self.proc.wait(10)
        self.log.close()
        self.cli(self.db, "remove-connection", "--id", self.provider)

    def version(self):
        return self.args.pygg_build or "unknown build"


class LiteLLMArm:
    port = 4001

    def __init__(self, args, name, model_name):
        self.args, self.name, self.model_name = args, name, model_name
        self.path = "y" if model_name == "haiku-inject" else "x"

    def start(self):
        subprocess.run(["docker", "rm", "-f", "pyggbench-litellm-replay"], capture_output=True)
        config = (HERE / "litellm-replay.yaml").resolve()
        cmd = ["docker", "run", "-d", "--name", "pyggbench-litellm-replay", "--network", "pyggbench", "-p",
               f"127.0.0.1:{self.port}:4000", "-v", f"{config}:/app/config.yaml:ro", self.args.litellm_image,
               "--config", "/app/config.yaml", "--port", "4000"]
        out = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, "MSYS_NO_PATHCONV": "1"})
        if out.returncode != 0:
            raise RuntimeError(out.stderr)
        deadline = time.time() + 120
        while time.time() < deadline:
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{self.port}/health/liveliness", timeout=2)
                return
            except Exception:
                time.sleep(1)
        raise RuntimeError("litellm did not start")

    def model(self):
        return self.model_name

    def key(self):
        return "sk-bench-master"

    def stop(self):
        logs = subprocess.run(["docker", "logs", "pyggbench-litellm-replay"], capture_output=True, text=True, encoding="utf-8", errors="replace")
        (Path(self.args.out) / f"{self.name}.container.log").write_text((logs.stdout or "")[-20000:] + (logs.stderr or "")[-20000:], encoding="utf-8")
        subprocess.run(["docker", "rm", "-f", "pyggbench-litellm-replay"], capture_output=True)

    def version(self):
        return self.args.litellm_image


def cost(u):
    return (u["input_tokens"] * PRICE["input"] + u["cache_read_input_tokens"] * PRICE["cache_read"]
            + u["cache_creation_input_tokens"] * PRICE["cache_write"] + 40 * PRICE["output"]) / 1e6


def run_arm(arm, args):
    arm.start()
    http_json("POST", f"{FAKE}/_admin/reset", {})
    statuses = []
    for body in session(args.turns):
        body["model"] = arm.model()
        status, raw = send(arm.port, arm.key(), body)
        statuses.append(status)
        if status != 200:
            print(f"  {arm.name}: HTTP {status}: {raw[:300]}", file=sys.stderr)
    # Output limits: the same short question without max_tokens, whole and streamed.
    probes = []
    for stream in (False, True):
        before = len([l for l in http_json("GET", f"{FAKE}/_admin/log").splitlines() if l.strip()])
        body = {"model": arm.model(), "messages": [{"role": "user", "content": "Say ok."}], "stream": stream}
        status, raw = send(arm.port, arm.key(), body)
        log = [json.loads(l) for l in http_json("GET", f"{FAKE}/_admin/log").splitlines() if l.strip()]
        probes.append({"stream": stream, "status": status,
                       "max_tokens_sent_upstream": log[before]["max_tokens"] if len(log) > before else None})
    log = [json.loads(l) for l in http_json("GET", f"{FAKE}/_admin/log").splitlines() if l.strip()]
    arm.stop()
    turns = [e for e in log if e.get("api") == "anthropic"][: args.turns]
    total = {k: sum(t["usage"][k] for t in turns) for k in ("input_tokens", "cache_read_input_tokens",
                                                            "cache_creation_input_tokens")}
    prompt = sum(total.values())
    summary = {
        "arm": arm.name, "version": arm.version(), "turns": len(turns), "http_ok": statuses.count(200),
        "prompt_tokens": prompt, **total,
        "share_read_from_cache": round(total["cache_read_input_tokens"] / prompt, 4) if prompt else None,
        "input_cost_usd": round(sum(cost(t["usage"]) - 40 * PRICE["output"] / 1e6 for t in turns), 6),
        "cost_usd_with_40_output_tokens_a_turn": round(sum(cost(t["usage"]) for t in turns), 6),
        "markers_per_turn": [t["markers"] for t in turns[:3]] + ["..."],
        "max_tokens_on_turns": sorted({str(t["max_tokens"]) for t in turns}),
        "output_limit_probes": probes,
    }
    return summary, turns


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--turns", type=int, default=15)
    ap.add_argument("--arms", default="pygg,pygg-cache-off,litellm,litellm-inject")
    ap.add_argument("--work", default=os.environ.get("PYGG_BENCH_WORK", str(HERE / ".work")))
    ap.add_argument("--pygg-exe", default=os.environ.get("PYGG_GATEWAY_EXE", "gateway"))
    ap.add_argument("--api", choices=["chat", "responses"], default="chat",
                    help="send the turns as Chat Completions (OpenCode and most agents) or Responses (Codex)")
    ap.add_argument("--pygg-build", help="the commit the gateway binary was built from, for the results")
    ap.add_argument("--litellm-image", default="ghcr.io/berriai/litellm:v1.106.2")
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    if args.api == "responses":
        API["path"] = "/v1/responses"
    arms = {"pygg": lambda: PyggArm(args, "pygg", False), "pygg-cache-off": lambda: PyggArm(args, "pygg-cache-off", True),
            "litellm": lambda: LiteLLMArm(args, "litellm", "haiku"),
            "litellm-inject": lambda: LiteLLMArm(args, "litellm-inject", "haiku-inject")}
    for name in args.arms.split(","):
        arm = arms[name]()
        print(f"[{name}]", flush=True)
        summary, turns = run_arm(arm, args)
        summary["at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        summary["api"] = args.api
        print("  " + json.dumps({k: summary[k] for k in ("http_ok", "prompt_tokens", "share_read_from_cache",
                                                         "input_cost_usd", "max_tokens_on_turns", "output_limit_probes")}))
        with open(out / "summary.jsonl", "a") as f:
            f.write(json.dumps(summary) + "\n")
        with open(out / f"{name}.{args.api}.turns.jsonl", "w") as f:
            for t in turns:
                f.write(json.dumps(t) + "\n")


if __name__ == "__main__":
    main()
