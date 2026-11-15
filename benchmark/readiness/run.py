"""Readiness benchmark: how close Pygg is to a public release, scored against the release gates in docs/ROADMAP.md.

One command, free or local traffic only (the fake provider in benchmark/resilience, and Ollama if it answers):

    python -I benchmark/readiness/run.py                 everything
    python -I benchmark/readiness/run.py --skip ui,npm   leave groups out (build, gateway, restart, overhead, setup,
                                                         auto, rust, ui, npm, ollama)

It builds the headless `gateway` and `pygg-cli` (release), starts the fake provider and a gateway on a scratch store,
and runs each check. Every check passes or fails with numbers. Outputs:

    benchmark/readiness/runs/<time>-<commit>/   results.json, scorecard.md, logs (git-ignored)
    benchmark/readiness/SCORECARD.md            the latest scorecard
    benchmark/readiness/trend.jsonl             one line per run, to compare runs over time

A gate is red when one of its checks failed, open when its checks pass but the gate still needs evidence this
harness cannot produce (listed), and green otherwise. Nothing here sends a prompt to a paid provider or a web session.
The fake keys are kept in the system's credential store while the run lasts and removed at the end.
"""

import argparse
import datetime as dt
import http.client
import json
import os
import re
import shutil
import socket
import sqlite3
import subprocess
import sys
import threading
import time
import tomllib
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
TAURI = ROOT / "apps" / "desktop" / "src-tauri"
FAKE = ROOT / "benchmark" / "resilience" / "fake_provider.py"
EXE = ".exe" if os.name == "nt" else ""
FAKE_PORT = 9930
GW_PORT = 8796
OLLAMA = "http://127.0.0.1:11434"
OLLAMA_MODEL = "qwen3:8b"

GATES = {
    "Parity": "Same model direct and through Pygg: caching, tools, streaming do not regress.",
    "Honesty": "Unknown cost stays Unknown; every attempt is in the ledger.",
    "Security": "No limit overrun through concurrency, no unconfirmed key switch, loopback only.",
    "Errors": "Provider errors run through the error policy as fixtures.",
    "Install": "Install, set-up and removal leave nothing behind.",
    "Interface": "ui-check passes; every screen clicked through; translations complete.",
    "Web sessions": "The guard stays on and cannot be turned off.",
    # Tracked, not release gates.
    "Performance": "What the gateway adds to a request (tracked, not a gate).",
    "Routing": "`auto` decisions (tracked, not a gate).",
}
RELEASE_GATES = ["Parity", "Honesty", "Security", "Errors", "Install", "Interface", "Web sessions"]

# Evidence a gate needs that this harness cannot produce (docs/ROADMAP.md); computed items are added at run time.
STATIC_OPEN = {
    "Parity": ["live parity per claimed cloud provider path (two turns, caching, tools, streaming): only the fake "
               "provider and local Ollama are measured here"],
    "Install": ["clean-machine NSIS install and uninstall", "signed installer (release only)"],
    "Interface": ["every screen clicked through in the native window", "D12 checks (scaling, snap layouts, Narrator "
                  "and NVDA, contrast themes)"],
}


# ---------- small helpers ----------

class Results:
    def __init__(self):
        self.checks = []

    def add(self, cid, gate, ok, numbers=None, detail=""):
        ok = None if ok is None else bool(ok)
        self.checks.append({"id": cid, "gate": gate, "ok": ok, "numbers": numbers or {}, "detail": detail})
        mark = {True: "PASS", False: "FAIL", None: "SKIP"}[ok]
        nums = " ".join(f"{k}={v}" for k, v in (numbers or {}).items())
        print(f"  {mark} {gate:<12} {cid:<28} {nums} {detail}".rstrip(), flush=True)

    def guard(self, cid, gate, fn):
        """Runs fn() -> (ok, numbers, detail); an exception fails the check with its message."""
        try:
            ok, numbers, detail = fn()
        except Exception as e:  # noqa: BLE001 - a broken check is a failed check, with the reason
            ok, numbers, detail = False, {}, f"error: {type(e).__name__}: {str(e)[:300]}"
        self.add(cid, gate, ok, numbers, detail)
        return ok


def req(method, port, path, body=None, headers=None, timeout=30, host="127.0.0.1"):
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    data = json.dumps(body).encode() if body is not None else None
    hdrs = {"content-type": "application/json", **(headers or {})}
    conn.request(method, path, body=data, headers=hdrs)
    r = conn.getresponse()
    raw = r.read()
    out = (r.status, {k.lower(): v for k, v in r.getheaders()}, raw)
    conn.close()
    return out


def sse(raw):
    events = []
    for line in raw.decode("utf-8", "replace").splitlines():
        if line.startswith("data:"):
            payload = line[5:].strip()
            if payload == "[DONE]":
                events.append("[DONE]")
                continue
            try:
                events.append(json.loads(payload))
            except json.JSONDecodeError:
                pass
    return events


def wait_port(port, seconds=30):
    end = time.time() + seconds
    while time.time() < end:
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.2)
    return False


def run(cmd, cwd=None, env=None, timeout=1800, log=None, shell=False):
    started = time.time()
    p = subprocess.run(cmd, cwd=cwd, env={**os.environ, **(env or {})}, capture_output=True, text=True,
                       encoding="utf-8", errors="replace", timeout=timeout, shell=shell)
    if log:
        Path(log).write_text(f"$ {cmd}\n\n{p.stdout}\n--- stderr ---\n{p.stderr}", encoding="utf-8")
    return p.returncode, p.stdout, p.stderr, round(time.time() - started, 1)


def vault_entries():
    """How many entries Pygg has in Windows' credential store (names only, never values); None elsewhere."""
    if os.name != "nt":
        return None
    out = subprocess.run(["cmdkey", "/list"], capture_output=True, text=True, errors="replace").stdout
    return sum(1 for line in out.splitlines() if "target=" in line.lower() and line.rstrip().endswith(".SmartTaskRouter"))


def git(*args):
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True).stdout.strip()


# ---------- the fake provider and the gateway ----------

class Fake:
    def __init__(self, logdir):
        self.proc = subprocess.Popen([sys.executable, "-I", str(FAKE), str(FAKE_PORT)],
                                     stdout=open(logdir / "fake.log", "w"), stderr=subprocess.STDOUT)
        if not wait_port(FAKE_PORT, 15):
            raise RuntimeError("the fake provider did not start")
        self.sent = 0

    def config(self, rules, latency_ms=20):
        req("POST", FAKE_PORT, "/_admin/config", {"latency_ms": latency_ms, "rules": rules})

    def reset(self):
        # Keeps count of what the gateway sent (not the direct requests of the cache check), for the ledger check.
        self.sent += len([e for e in self.log() if e.get("provider") != "cdirect"])
        req("POST", FAKE_PORT, "/_admin/reset", {})

    def log(self):
        _, _, raw = req("GET", FAKE_PORT, "/_admin/log")
        return [json.loads(line) for line in raw.decode().splitlines() if line.strip()]

    def stop(self):
        self.proc.kill()


class Gateway:
    """The headless gateway on a scratch store, set up with fake connections."""

    def __init__(self, exe, work):
        self.exe, self.work = exe, work
        self.db = work / "store.db"
        self.proc = None
        self.ids = {}
        self.tokens = {}
        self.env = {}

    def cli(self, *argv, env=None, check=True):
        p = subprocess.run([str(self.exe), "--db", str(self.db), *argv], capture_output=True, text=True,
                           env={**os.environ, **self.env, **(env or {})}, timeout=120)
        if check and p.returncode != 0:
            raise RuntimeError(f"gateway {argv[0]}: {p.stderr.strip()[:300]}")
        return p

    def add(self, name, preset, path=None, key=None, endpoint=None, billing="paid"):
        argv = ["add-connection", "--preset", preset, "--label", f"Readiness {name}", "--billing", billing,
                "--endpoint", endpoint or f"http://127.0.0.1:{FAKE_PORT}/{path}/v1"]
        if key:
            var = f"PYGG_READINESS_{key.upper().replace('-', '_')}"
            self.env[var] = key
            argv += ["--key-env", var]
        out = self.cli(*argv).stdout
        self.ids[name] = out.split("Added connection ", 1)[1].split(" ", 1)[0]
        return self.ids[name]

    def model(self, ref, *flags):
        self.cli("model", "--model", ref, *flags, "--price-source", "readiness benchmark")

    def setup(self):
        if self.work.exists():
            shutil.rmtree(self.work)
        self.work.mkdir(parents=True)
        # One preset kind per connection: two connections of one kind share a quota group.
        for name, preset, path in [("a", "deepinfra", "a"), ("b", "together", "b"), ("r", "nebius", "r"),
                                   ("f429", "fireworks", "f429"), ("f401", "cerebras", "f401"),
                                   ("f404", "sambanova", "f404"), ("x500", "siliconflow", "x500"),
                                   ("hang", "xai", "hang"), ("cap", "mistral", "cap")]:
            self.add(name, preset, path, f"key-{name}")
        self.add("c", "anthropic", "c", "key-c")
        for k in ("r2", "r3"):
            self.env[f"PYGG_READINESS_KEY_{k.upper()}"] = f"key-{k}"
        self.cli("key", "--id", self.ids["r"], "--label", "Account 2", "--key-env", "PYGG_READINESS_KEY_R2",
                 "--separate-account", "yes")
        self.cli("key", "--id", self.ids["r"], "--label", "Account 3", "--key-env", "PYGG_READINESS_KEY_R3",
                 "--separate-account", "yes", "--strategy", "round_robin")
        std = ["--tier", "T2", "--context", "128000", "--max-output", "8192", "--tools", "yes"]
        for name in ("a", "b", "r", "f429", "f401", "f404", "x500", "hang", "cap"):
            self.model(f"{self.ids[name]}/fake-model", *std, "--price-in", "1", "--price-out", "2")
        self.model(f"{self.ids['a']}/fake-cheap", "--tier", "T1", "--context", "64000", "--max-output", "4096",
                   "--tools", "yes", "--price-in", "0.1", "--price-out", "0.2")
        self.model(f"{self.ids['b']}/fake-strong", "--tier", "T4", "--context", "200000", "--max-output", "16000",
                   "--tools", "yes", "--price-in", "5", "--price-out", "15")
        self.model(f"{self.ids['c']}/claude-haiku-5-5", "--tier", "T2", "--context", "200000", "--max-output",
                   "64000", "--tools", "yes", "--price-in", "3", "--price-out", "15", "--price-cached", "0.3",
                   "--price-cache-write", "3.75")
        for combo, first in (("fb429", "f429"), ("fb401", "f401"), ("fb404", "f404"), ("fb500", "x500")):
            self.cli("combo", "--name", combo, "--member", f"{self.ids[first]}/fake-model", "--member",
                     f"{self.ids['b']}/fake-model")
        for client in ("bench", "capped"):
            out = self.cli("create-client", "--name", client).stdout
            self.tokens[client] = out.strip().splitlines()[-1].strip()
        self.cli("client", "--name", "capped", "--monthly-usd", "0.005", "--strict", "yes")

    def start(self, log_name="gateway.log"):
        self.log = open(self.work / log_name, "a")
        self.proc = subprocess.Popen([str(self.exe), "--db", str(self.db), "serve", "--port", str(GW_PORT)],
                                     stdout=self.log, stderr=subprocess.STDOUT, env={**os.environ, **self.env})
        started = time.time()
        if not wait_port(GW_PORT, 30):
            raise RuntimeError("the gateway did not start")
        return round((time.time() - started) * 1000)

    def kill(self):
        if self.proc:
            self.proc.kill()
            self.proc.wait(10)
            self.proc = None

    def stop(self):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
            self.proc = None

    def teardown(self):
        for cid in self.ids.values():
            self.cli("remove-connection", "--id", cid, check=False)

    def q(self, sql, *args):
        con = sqlite3.connect(f"file:{self.db}?mode=ro", uri=True, timeout=10)
        try:
            return con.execute(sql, args).fetchall()
        finally:
            con.close()

    def post(self, path, body, client="bench", headers=None, timeout=30, anthropic=False):
        auth = {"x-api-key": self.tokens[client], "anthropic-version": "2023-06-01"} if anthropic else \
            {"authorization": f"Bearer {self.tokens[client]}"}
        return req("POST", GW_PORT, path, body, {**auth, **(headers or {})}, timeout)


def chat(model, rid, stream=False, **extra):
    body = {"model": model, "messages": [{"role": "user", "content": f"say ok rid={rid}"}], **extra}
    if stream:
        body.update(stream=True, stream_options={"include_usage": True})
    return body


TOOL_OAI = [{"type": "function", "function": {"name": "read_file", "description": "Read a file",
                                               "parameters": {"type": "object", "properties": {"path": {"type": "string"}},
                                                              "required": ["path"]}}}]
TOOL_ANT = [{"name": "read_file", "description": "Read a file",
             "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}]


def text_of_chat(status, raw, stream):
    if stream:
        events = sse(raw)
        text = "".join((e.get("choices") or [{}])[0].get("delta", {}).get("content") or ""
                       for e in events if isinstance(e, dict) and e.get("choices"))
        return text, "[DONE]" in events
    body = json.loads(raw)
    return body["choices"][0]["message"].get("content") or "", True


# ---------- the checks ----------

def gateway_checks(R, gw, fake):
    a, b = gw.ids["a"], gw.ids["b"]

    def openai(stream):
        def f():
            s, h, raw = gw.post("/v1/chat/completions", chat(f"{a}/fake-model", f"oai{int(stream)}", stream))
            text, done = text_of_chat(s, raw, stream)
            ok = s == 200 and f"rid=oai{int(stream)}" in text and done
            return ok, {"status": s, "route": h.get("x-pygg-route", "")[:40]}, "" if ok else raw[:200].decode("utf-8", "replace")
        return f

    R.guard("openai_chat_whole", "Parity", openai(False))
    R.guard("openai_chat_stream", "Parity", openai(True))

    def anthropic(stream):
        def f():
            body = {"model": f"{a}/fake-model", "max_tokens": 200, "stream": stream,
                    "messages": [{"role": "user", "content": f"say ok rid=ant{int(stream)}"}]}
            s, _, raw = gw.post("/v1/messages", body, anthropic=True)
            if stream:
                text = raw.decode("utf-8", "replace")
                ok = s == 200 and f"rid=ant{int(stream)}" in text and "message_stop" in text
            else:
                j = json.loads(raw)
                ok = s == 200 and j.get("type") == "message" and f"rid=ant0" in j["content"][0].get("text", "")
            return ok, {"status": s}, "" if ok else raw[:200].decode("utf-8", "replace")
        return f

    R.guard("anthropic_messages_whole", "Parity", anthropic(False))
    R.guard("anthropic_messages_stream", "Parity", anthropic(True))

    def responses():
        s, _, raw = gw.post("/v1/responses", {"model": f"{a}/fake-model", "input": "say ok rid=resp1"})
        ok = s == 200 and "rid=resp1" in raw.decode("utf-8", "replace")
        return ok, {"status": s}, "" if ok else raw[:200].decode("utf-8", "replace")

    R.guard("responses_whole", "Parity", responses)

    def tools_openai(stream):
        def f():
            body = chat(f"{a}/fake-model", f"tool{int(stream)}", stream, tools=TOOL_OAI)
            body["messages"][0]["content"] += " use a tool"
            s, _, raw = gw.post("/v1/chat/completions", body)
            if stream:
                events = [e for e in sse(raw) if isinstance(e, dict) and e.get("choices")]
                name = "".join(((tc.get("function") or {}).get("name") or "")
                               for e in events for tc in (e["choices"][0].get("delta", {}).get("tool_calls") or []))
                args = "".join(((tc.get("function") or {}).get("arguments") or "")
                               for e in events for tc in (e["choices"][0].get("delta", {}).get("tool_calls") or []))
            else:
                calls = json.loads(raw)["choices"][0]["message"].get("tool_calls") or []
                name = calls[0]["function"]["name"] if calls else ""
                args = calls[0]["function"]["arguments"] if calls else ""
            ok = s == 200 and name == "read_file" and json.loads(args or "{}").get("path") == "README.md"
            return ok, {"status": s, "tool": name}, ""
        return f

    R.guard("tools_openai_whole", "Parity", tools_openai(False))
    R.guard("tools_openai_stream", "Parity", tools_openai(True))

    def tools_anthropic(stream):
        def f():
            body = {"model": f"{a}/fake-model", "max_tokens": 200, "stream": stream, "tools": TOOL_ANT,
                    "messages": [{"role": "user", "content": f"rid=tant{int(stream)} use a tool"}]}
            s, _, raw = gw.post("/v1/messages", body, anthropic=True)
            if stream:
                text = raw.decode("utf-8", "replace")
                blocks, partial = [], ""
                for line in text.splitlines():
                    if line.startswith("data:"):
                        e = json.loads(line[5:])
                        if e.get("type") == "content_block_start":
                            blocks.append(e["content_block"])
                        if e.get("type") == "content_block_delta" and e["delta"].get("type") == "input_json_delta":
                            partial += e["delta"].get("partial_json", "")
                use = next((x for x in blocks if x.get("type") == "tool_use"), None)
                inp = json.loads(partial or "{}")
            else:
                j = json.loads(raw)
                use = next((x for x in j.get("content", []) if x.get("type") == "tool_use"), None)
                inp = (use or {}).get("input") or {}
            ok = s == 200 and use is not None and use.get("name") == "read_file" and inp.get("path") == "README.md"
            return ok, {"status": s}, "" if ok else raw[:300].decode("utf-8", "replace")
        return f

    R.guard("tools_anthropic_whole", "Parity", tools_anthropic(False))
    R.guard("tools_anthropic_stream", "Parity", tools_anthropic(True))

    def cache():
        """Three turns on the Anthropic surface with a cached system prompt, direct to the fake and through Pygg."""
        system = [{"type": "text", "text": "You are a careful coding agent. " * 300, "cache_control": {"type": "ephemeral"}}]

        def turns(send):
            msgs, reads = [], []
            for i in range(3):
                msgs.append({"role": "user", "content": f"turn {i} rid=cache{i}"})
                u = send({"model": "claude-haiku-5-5", "max_tokens": 100, "system": system, "messages": msgs})
                reads.append(u.get("cache_read_input_tokens", 0))
                msgs.append({"role": "assistant", "content": "ok"})
            return reads

        fake.reset()

        def direct(body):
            return json.loads(req("POST", FAKE_PORT, "/cdirect/v1/messages", body)[2])["usage"]

        def through(body):
            body = {**body, "model": f"{gw.ids['c']}/claude-haiku-5-5"}
            s, _, raw = gw.post("/v1/messages", body, anthropic=True)
            if s != 200:
                raise RuntimeError(f"status {s}: {raw[:200]}")
            return json.loads(raw)["usage"]

        d, p = turns(direct), turns(through)
        ok = p[1] > 0 and p[2] > 0 and p == d
        return ok, {"direct_cache_reads": d, "pygg_cache_reads": p}, ""

    R.guard("anthropic_cache_parity", "Parity", cache)

    def session_affinity():
        fake.reset()
        r = gw.ids["r"]
        for i in range(6):
            gw.post("/v1/chat/completions", chat(f"{r}/fake-model", f"s{i}"), headers={"x-pygg-session": "readiness-1"})
        pinned = {e["key"] for e in fake.log()}
        fake.reset()
        for i in range(6):
            gw.post("/v1/chat/completions", chat(f"{r}/fake-model", f"n{i}"))
        spread = {e["key"] for e in fake.log()}
        ok = len(pinned) == 1 and len(spread) >= 2
        return ok, {"keys_one_session": len(pinned), "keys_no_session": len(spread)}, ""

    R.guard("session_affinity_ring", "Security", session_affinity)

    def ring_needs_confirmation():
        p = gw.cli("key", "--id", gw.ids["a"], "--label", "Unconfirmed", "--key-env", "PYGG_READINESS_KEY_R2",
                   check=False)
        return p.returncode != 0, {"exit": p.returncode}, ""

    R.guard("ring_key_needs_confirmation", "Security", ring_needs_confirmation)

    def fallback(combo, rule, expect_status):
        def f():
            fake.reset()
            fake.config({combo.replace("fb", "f") if combo != "fb500" else "x500": rule})
            s, h, raw = gw.post("/v1/chat/completions", chat(combo, combo))
            log = fake.log()
            text = raw.decode("utf-8", "replace")
            ok = s == 200 and "via=b/" in text and h.get("x-pygg-attempts") == "2" and \
                [e["outcome"] for e in log] == [f"http_{expect_status}", "ok"]
            fake.config({})
            return ok, {"status": s, "attempts": h.get("x-pygg-attempts"),
                        "upstream": ",".join(e["outcome"] for e in log)}, "" if ok else text[:200]
        return f

    R.guard("fallback_on_429", "Errors", fallback("fb429", {"mode": "429"}, 429))
    R.guard("fallback_on_401", "Errors", fallback("fb401", {"mode": "401"}, 401))
    R.guard("fallback_on_404", "Errors", fallback("fb404", {"mode": "404"}, 404))

    def unknown_cost():
        fake.reset()
        fake.config({"x500": {"mode": "500"}})
        s, _, _ = gw.post("/v1/chat/completions", chat(f"{gw.ids['x500']}/fake-model", "u500"))
        fake.config({})
        rows = gw.q("SELECT outcome, cost_status, http_status FROM usage_events WHERE provider = ? "
                    "ORDER BY created_at DESC LIMIT 1", gw.ids["x500"])
        ok = s >= 500 and bool(rows) and rows[0][1] == "unknown"
        return ok, {"client_status": s, "ledger": "/".join(map(str, rows[0])) if rows else "none"}, ""

    R.guard("server_error_cost_unknown", "Honesty", unknown_cost)

    def budget_concurrency():
        fake.reset()
        fake.config({"cap": {"mode": "hang", "seconds": 1.5}})
        statuses = []

        def one(i):
            s, _, _ = gw.post("/v1/chat/completions", chat(f"{gw.ids['cap']}/fake-model", f"cap{i}", max_tokens=1000),
                              client="capped", timeout=30)
            statuses.append(s)

        threads = [threading.Thread(target=one, args=(i,)) for i in range(8)]
        [t.start() for t in threads]
        [t.join() for t in threads]
        sent = len(fake.log())
        fake.config({})
        spent = gw.q("SELECT COALESCE(SUM(cost_micro_usd), 0) FROM usage_events u JOIN gateway_clients c "
                     "ON c.id = u.client_id WHERE c.name = 'capped'")[0][0]
        held = gw.q("SELECT COUNT(*) FROM budget_reservations")[0][0]
        served = statuses.count(200)
        refused = sum(1 for s in statuses if 400 <= s < 500)
        # The strict limit ($0.005) fits two requests at their largest possible cost ($0.002 each, 1000 tokens out).
        ok = served == sent and 1 <= served <= 2 and refused == 8 - served and spent <= 5000 and held == 0
        return ok, {"served": served, "refused": refused, "sent_upstream": sent, "spent_micro_usd": spent}, ""

    R.guard("strict_budget_concurrency", "Security", budget_concurrency)

    def loopback():
        ips = sorted({ai[4][0] for ai in socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET)
                      if not ai[4][0].startswith("127.")})
        if not ips:
            return None, {}, "no LAN address on this computer"
        reached = []
        for ip in ips:
            try:
                with socket.create_connection((ip, GW_PORT), 1.5):
                    reached.append(ip)
            except OSError:
                pass
        return not reached, {"lan_addresses_tried": len(ips), "reached": len(reached)}, ""

    R.guard("loopback_only", "Security", loopback)

    def ledger_whole():
        """Every request the fake provider got from the gateway has exactly one ledger entry, and none is unsettled."""
        fake.reset()
        rows = gw.q("SELECT COUNT(*) FROM usage_events")[0][0]
        open_ = gw.q("SELECT COUNT(*) FROM budget_reservations")[0][0]
        return open_ == 0 and rows == fake.sent, {"upstream_requests": fake.sent, "ledger_rows": rows,
                                                   "unsettled": open_}, ""

    R.guard("ledger_settled", "Honesty", ledger_whole)


def restart_checks(R, gw, fake):
    def kill_restart():
        fake.reset()
        fake.config({"hang": {"mode": "hang", "seconds": 4}})
        before = gw.q("SELECT COUNT(*) FROM usage_events WHERE outcome = 'interrupted'")[0][0]
        errors = []

        def one(i):
            try:
                gw.post("/v1/chat/completions", chat(f"{gw.ids['hang']}/fake-model", f"k{i}"), timeout=20)
            except Exception as e:  # noqa: BLE001 - the gateway is killed under it
                errors.append(type(e).__name__)

        threads = [threading.Thread(target=one, args=(i,)) for i in range(6)]
        [t.start() for t in threads]
        time.sleep(1.5)
        journaled = gw.q("SELECT COUNT(*) FROM budget_reservations")[0][0]
        gw.kill()
        [t.join() for t in threads]
        time.sleep(3.5)  # the fake finishes the hung requests and logs them
        upstream = len([e for e in fake.log() if e.get("provider") == "hang"])
        fake.config({})
        restart_ms = gw.start("gateway-restarted.log")
        after = gw.q("SELECT COUNT(*) FROM usage_events WHERE outcome = 'interrupted'")[0][0]
        unknown = gw.q("SELECT COUNT(*) FROM usage_events WHERE outcome = 'interrupted' AND cost_status <> 'unknown'")[0][0]
        left = gw.q("SELECT COUNT(*) FROM budget_reservations")[0][0]
        s, _, _ = gw.post("/v1/chat/completions", chat(f"{gw.ids['a']}/fake-model", "afterrestart"))
        interrupted = after - before
        ok = journaled == 6 and upstream == 6 and interrupted == 6 and unknown == 0 and left == 0 and s == 200 \
            and len(errors) == 6
        return ok, {"in_flight": 6, "journaled_before_kill": journaled, "upstream_saw": upstream,
                    "interrupted_entries": interrupted, "restart_ms": restart_ms, "after_restart": s}, ""

    R.guard("kill_restart_ledger_whole", "Honesty", kill_restart)


def auto_checks(R, gw):
    def decide(*flags):
        out = gw.cli("auto", "--client", "bench", *flags).stdout
        ranked = re.findall(r"^\s*\d+\. (\S+)", out, re.M)
        excluded = re.findall(r"^    (\S+): (.*)$", out, re.M)
        return ranked, dict(excluded)

    a = gw.ids["a"]

    def cheap():
        first = [decide("--alias", "auto/cheap", "--tools", "yes")[0][:1] for _ in range(3)]
        top = first[0][0] if first[0] else ""
        gw.model(f"{a}/fake-cheap", "--tools", "no")
        ranked, excluded = decide("--alias", "auto/cheap", "--tools", "yes")
        gw.model(f"{a}/fake-cheap", "--tools", "yes")
        ok = top == f"{a}/fake-cheap" and all(f == first[0] for f in first) and f"{a}/fake-cheap" not in ranked \
            and f"{a}/fake-cheap" in excluded
        return ok, {"auto_cheap_pick": top.split("/")[-1], "without_tools_pick": (ranked[:1] or [""])[0].split("/")[-1],
                    "excluded_reason": excluded.get(f"{a}/fake-cheap", "")[:50]}, ""

    R.guard("auto_cheap_decision", "Routing", cheap)

    def big_prompt():
        ranked, excluded = decide("--alias", "auto", "--prompt", "150000")
        small = [m for m in ranked if m.endswith("/fake-model") or m.endswith("/fake-cheap")]
        ok = bool(ranked) and not small
        return ok, {"ranked": len(ranked), "pick": (ranked[:1] or [""])[0].split("/")[-1]}, ""

    R.guard("auto_context_fit", "Routing", big_prompt)

    def live():
        s, h, raw = gw.post("/v1/chat/completions", chat("auto/cheap", "autolive"))
        route = h.get("x-pygg-route", "")
        ok = s == 200 and "fake-cheap" in route
        return ok, {"status": s, "route": route[:60]}, ""

    R.guard("auto_cheap_request", "Routing", live)


def setup_checks(R, cli_exe, gw, work):
    homes = work / "homes"
    if homes.exists():
        shutil.rmtree(homes)
    codex, xdg, claude = homes / "codex", homes / "xdg", homes / "claude"
    for d in (codex, xdg / "opencode", claude):
        d.mkdir(parents=True)
    codex_seed = 'model = "gpt-5"\n\n[profiles.mine]\nmodel = "o3"\n'
    (codex / "config.toml").write_text(codex_seed, encoding="utf-8")
    oc_seed = {"$schema": "https://opencode.ai/config.json", "model": "anthropic/claude-x", "provider": {"mine": {"npm": "x"}}}
    (xdg / "opencode" / "opencode.json").write_text(json.dumps(oc_seed, indent=2), encoding="utf-8")
    env = {"CODEX_HOME": str(codex), "XDG_CONFIG_HOME": str(xdg), "CLAUDE_CONFIG_DIR": str(claude)}

    def pygg(*argv):
        return run([str(cli_exe), *argv, "--db", str(gw.db)], env=env, timeout=120)

    def active_clients():
        return gw.q("SELECT COUNT(*) FROM gateway_clients WHERE revoked_at IS NULL AND deleted_at IS NULL")[0][0]

    def tool(name, path, parse, seed_parsed):
        def f():
            base = active_clients()
            code1, out1, err1, t1 = pygg("setup", name)
            text = path.read_text(encoding="utf-8")
            after = parse(text)
            kept = all(after.get(k) == v for k, v in seed_parsed.items() if k != "provider") and \
                all(k in after.get("provider", {}) for k in seed_parsed.get("provider", {}))
            has = "pygg" in text.lower()
            made = active_clients() - base
            code2, _, err2, t2 = pygg("setup", name, "--remove")
            back = parse(path.read_text(encoding="utf-8"))
            gone = "pygg" not in json.dumps(back).lower()
            restored = back == seed_parsed
            revoked = active_clients() == base
            backups = list((gw.db.parent / "integration-backups").rglob("*"))
            ok = code1 == 0 and has and kept and made == 1 and code2 == 0 and gone and restored and revoked and backups
            return ok, {"setup_s": t1, "remove_s": t2, "key_made": made, "restored": restored, "backups": len(backups)}, \
                "" if ok else (err1 or err2)[:200]
        return f

    R.guard("setup_codex", "Install", tool("codex", codex / "config.toml", tomllib.loads, tomllib.loads(codex_seed)))
    R.guard("setup_opencode", "Install", tool("opencode", xdg / "opencode" / "opencode.json", json.loads, oc_seed))

    def claude_launcher():
        base = active_clients()
        code1, _, err1, t1 = pygg("setup", "claude")
        _, listing, _, _ = pygg("setup")
        set_up = bool(re.search(r"^claude\s+\S+\s+set up", listing, re.M))
        made = active_clients() - base
        code2, _, err2, t2 = pygg("setup", "claude", "--remove")
        _, listing2, _, _ = pygg("setup")
        removed = bool(re.search(r"^claude\s+\S+\s+not set up", listing2, re.M))
        untouched = not any(claude.iterdir())
        ok = code1 == 0 and set_up and made == 1 and code2 == 0 and removed and active_clients() == base and untouched
        return ok, {"setup_s": t1, "remove_s": t2, "key_made": made, "files_written": 0 if untouched else 1}, \
            "" if ok else (err1 or err2 or listing)[:200]

    R.guard("setup_claude_launcher", "Install", claude_launcher)


def overhead_checks(R, logdir):
    out = logdir / "overhead.json"
    code, stdout, stderr, secs = run(["cargo", "test", "--release", "--no-default-features", "--lib",
                                      "gateway::tests::overhead", "--", "--ignored", "--nocapture"],
                                     cwd=TAURI, env={"PYGG_OVERHEAD_OUT": str(out)}, log=logdir / "overhead.log")
    if code != 0 or not out.exists():
        R.add("gateway_overhead", "Performance", False, {"seconds": secs}, (stderr or stdout)[-300:])
        return
    o = json.loads(out.read_text())
    sc = {s["name"]: s for s in o["scenarios"]}
    short = round(sc["short prompt"]["addedP50Ms"], 2)
    big = round(sc["400 kB prompt"]["addedP50Ms"], 2)
    rps = round([x for x in o["concurrent16"] if x["via"] == "gateway"][0]["requestsPerSecond"])
    # Thresholds: about twice the 2026-11-14 figures (3.8 ms, 5.1 ms, 232 requests/s), so a regression shows.
    ok = short <= 8 and big <= 12 and rps >= 120
    R.add("gateway_overhead", "Performance", ok, {"added_p50_short_ms": short, "added_p50_400kB_ms": big,
                                                  "rps_16_clients": rps})


def rust_checks(R, logdir):
    code, out, err, secs = run(["cargo", "test", "--no-default-features", "--test", "provider_errors"], cwd=TAURI,
                               log=logdir / "provider-errors.log")
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    R.add("provider_error_fixtures", "Errors", code == 0,
          {"passed": int(m.group(1)) if m else 0, "failed": int(m.group(2)) if m else -1, "seconds": secs})
    code, out, err, secs = run(["cargo", "test", "--no-default-features", "--lib", "websession::"], cwd=TAURI,
                               log=logdir / "websession.log")
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    R.add("web_guard_tests", "Web sessions", code == 0,
          {"passed": int(m.group(1)) if m else 0, "failed": int(m.group(2)) if m else -1, "seconds": secs})


def ui_checks(R, logdir):
    vite = None
    if not wait_port(1431, 0.5):
        vite = subprocess.Popen("pnpm --filter @pygg/desktop exec vite --host 127.0.0.1 --port 1431 --strictPort",
                                cwd=ROOT, shell=True, stdout=open(logdir / "vite.log", "w"), stderr=subprocess.STDOUT)
        if not wait_port(1431, 60):
            R.add("ui_check", "Interface", False, {}, "the preview did not start")
            return
    try:
        code, out, err, secs = run("pnpm ui-check", cwd=ROOT, shell=True, log=logdir / "ui-check.log", timeout=1200)
        m = re.search(r"^(\d+) layout or naming problems?", out, re.M)
        broken = sorted({line.split(":")[0] for line in out.splitlines() if "the steps failed" in line})
        R.add("ui_check", "Interface", code == 0, {"exit": code, "findings": int(m.group(1)) if m else 0,
                                                    "seconds": secs},
              "" if code == 0 else ("steps failed: " + "; ".join(broken) if broken else "see ui-check.log"))
    finally:
        if vite:
            subprocess.run(f"taskkill /PID {vite.pid} /T /F", shell=True, capture_output=True) if os.name == "nt" \
                else vite.kill()


def npm_checks(R, work, logdir):
    out = work / "npm"
    if out.exists():
        shutil.rmtree(out)
    code, stdout, stderr, secs = run(["node", "scripts/npm-pack.mjs", "--out", str(out)], cwd=ROOT,
                                     log=logdir / "npm-pack.log")
    tgz = sorted(out.glob("*.tgz"))
    if code != 0 or len(tgz) < 2:
        R.add("npm_install_back", "Install", False, {"pack_s": secs}, (stderr or stdout)[-200:])
        return
    prefix = work / "npm-install"
    if prefix.exists():
        shutil.rmtree(prefix)
    prefix.mkdir()
    (prefix / "package.json").write_text('{"name":"readiness-install","private":true}', encoding="utf-8")
    code2, o2, e2, secs2 = run(f"npm install --no-audit --no-fund --offline {' '.join(str(t) for t in tgz)}",
                               cwd=prefix, shell=True, log=logdir / "npm-install.log", timeout=600)
    pygg = prefix / "node_modules" / ".bin" / ("pygg.cmd" if os.name == "nt" else "pygg")
    code3, o3, e3, _ = run(f'"{pygg}" --version', shell=True, timeout=60) if pygg.exists() else (1, "", "no pygg", 0)
    npm_version = json.loads((ROOT / "packages/npm/pygg/package.json").read_text())["version"]
    crate = re.search(r'^version\s*=\s*"([^"]+)"', (TAURI / "Cargo.toml").read_text(), re.M).group(1)
    # The launcher must run the native program it brought; the package's prerelease tag is not in the program's own.
    ok = code2 == 0 and code3 == 0 and o3.strip() == f"pygg-cli {crate}" and npm_version.startswith(crate)
    R.add("npm_install_back", "Install", ok, {"pack_s": secs, "install_s": secs2, "packages": len(tgz),
                                               "npm_version": npm_version, "reports": o3.strip()[:30]},
          "" if ok else (e2 or e3 or o3)[-200:])


def ollama_checks(R, gw):
    try:
        tags = json.loads(urllib.request.urlopen(f"{OLLAMA}/api/tags", timeout=3).read())
    except Exception:  # noqa: BLE001
        R.add("ollama_parity", "Parity", None, {}, "Ollama does not answer on 11434")
        return
    if not any(m["name"] == OLLAMA_MODEL for m in tags.get("models", [])):
        R.add("ollama_parity", "Parity", None, {}, f"{OLLAMA_MODEL} is not pulled")
        return

    def f():
        cid = gw.add("ollama", "ollama", endpoint=f"{OLLAMA}/v1", billing="local")
        body = {"messages": [{"role": "system", "content": "Answer with one word."},
                             {"role": "user", "content": "/no_think Name a colour."}],
                "max_tokens": 64, "temperature": 0, "seed": 7}
        t = time.time()
        d = json.loads(urllib.request.urlopen(urllib.request.Request(
            f"{OLLAMA}/v1/chat/completions", json.dumps({**body, "model": OLLAMA_MODEL}).encode(),
            {"content-type": "application/json"}), timeout=300).read())
        direct_s = time.time() - t
        t = time.time()
        s, h, raw = gw.post("/v1/chat/completions", {**body, "model": f"{cid}/{OLLAMA_MODEL}"}, timeout=300)
        pygg_s = time.time() - t
        p = json.loads(raw)
        led = gw.q("SELECT input_tokens, cost_status, billing FROM usage_events WHERE provider = ? "
                   "ORDER BY created_at DESC LIMIT 1", cid)
        same_prompt = d["usage"]["prompt_tokens"] == p["usage"]["prompt_tokens"]
        ok = s == 200 and same_prompt and bool(led) and led[0][0] == p["usage"]["prompt_tokens"]
        return ok, {"prompt_tokens_direct": d["usage"]["prompt_tokens"], "prompt_tokens_pygg": p["usage"]["prompt_tokens"],
                    "ledger_tokens": led[0][0] if led else None, "direct_s": round(direct_s, 2),
                    "pygg_s": round(pygg_s, 2)}, ""

    R.guard("ollama_parity", "Parity", f)


# ---------- computed open items ----------

def open_items():
    items = {g: list(v) for g, v in STATIC_OPEN.items()}
    fixtures = [p.stem for p in (TAURI / "tests" / "fixtures" / "provider-errors").glob("*.json")]
    missing = [k for k in ("403", "429", "quota", "timeout") if not any(k in f for f in fixtures)]
    if missing:
        items.setdefault("Errors", []).append(f"real provider answers not yet captured as fixtures: {', '.join(missing)} "
                                              f"({len(fixtures)} fixtures today)")
    src = ROOT / "apps" / "desktop" / "src"
    used = set()
    for p in src.rglob("*.ts*"):
        if "locales" in p.parts:
            continue
        for m in re.finditer(r"""\b(?:t|plural)\(\s*(['"])((?:\\.|(?!\1).)+)\1""", p.read_text(encoding="utf-8")):
            used.add(m.group(2).replace("\\'", "'").replace('\\"', '"'))
    vi = set()
    for p in (src / "locales" / "vi").glob("*.ts"):
        for m in re.finditer(r"""^\s*(['"])((?:\\.|(?!\1).)+)\1\s*:""", p.read_text(encoding="utf-8"), re.M):
            vi.add(m.group(2).replace("\\'", "'").replace('\\"', '"'))
    covered = len(used & vi)
    pct = round(100 * covered / len(used), 1) if used else 0
    if pct < 100:
        items.setdefault("Interface", []).append(f"Vietnamese strings {pct}% complete ({covered} of {len(used)} found "
                                                 f"in t() calls; translation is scheduled last)")
    return items, {"vi_strings_pct": pct, "error_fixtures": len(fixtures)}


# ---------- the scorecard ----------

def scorecard(meta, checks, items, metrics, previous):
    lines = [f"# Readiness scorecard", "",
             f"Run {meta['time']} on commit `{meta['commit']}`{' (with uncommitted changes)' if meta['dirty'] else ''}, "
             f"{meta['seconds']} s. Free and local traffic only. Made by `python -I benchmark/readiness/run.py`; "
             "what each gate means is in [docs/ROADMAP.md](../../docs/ROADMAP.md#release-gates).", "",
             "| Gate | State | Checks passed | Still needed |", "| --- | --- | --- | --- |"]
    gates = {}
    for g in GATES:
        cs = [c for c in checks if c["gate"] == g]
        failed = [c for c in cs if c["ok"] is False]
        passed = [c for c in cs if c["ok"] is True]
        need = items.get(g, [])
        state = "red" if failed else ("open" if need else ("green" if passed else "not run"))
        gates[g] = state
        label = g if g in RELEASE_GATES else f"{g} (tracked)"
        lines.append(f"| {label} | **{state}** | {len(passed)} of {len(cs)} | {'; '.join(need) or '-'} |")
    lines += ["", "## Checks", "", "| Gate | Check | Result | Numbers |", "| --- | --- | --- | --- |"]
    prev = {c["id"]: c["ok"] for c in (previous or {}).get("checks_list", [])}
    for c in checks:
        res = {True: "pass", False: "**FAIL**", None: "skipped"}[c["ok"]]
        if c["id"] in prev and prev[c["id"]] != c["ok"]:
            res += f" (was {({True: 'pass', False: 'fail', None: 'skipped'})[prev[c['id']]]})"
        nums = ", ".join(f"{k} {v}" for k, v in c["numbers"].items())
        if c["detail"]:
            nums += (" · " if nums else "") + c["detail"].replace("|", "/").replace("\n", " ")[:160]
        lines.append(f"| {c['gate']} | `{c['id']}` | {res} | {nums} |")
    lines += ["", f"Metrics: {', '.join(f'{k} {v}' for k, v in metrics.items())}.", ""]
    return "\n".join(lines), gates


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--skip", default="", help="comma-separated groups to leave out")
    ap.add_argument("--only", default="", help="comma-separated groups to run (default: all)")
    ap.add_argument("--no-trend", action="store_true", help="do not append to trend.jsonl or write SCORECARD.md")
    args = ap.parse_args()
    groups = ["build", "gateway", "restart", "auto", "setup", "ollama", "overhead", "rust", "ui", "npm"]
    want = [g for g in (args.only.split(",") if args.only else groups) if g and g not in args.skip.split(",")]

    started = time.time()
    commit = git("rev-parse", "--short", "HEAD")
    dirty = bool(git("status", "--porcelain", "--untracked-files=no"))
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H%MZ")
    logdir = HERE / "runs" / f"{stamp}-{commit}"
    logdir.mkdir(parents=True, exist_ok=True)
    work = HERE / ".work"
    work.mkdir(exist_ok=True)
    R = Results()
    print(f"readiness run {stamp} on {commit}{' (dirty)' if dirty else ''}: {', '.join(want)}", flush=True)

    release = TAURI / "target" / "release"
    gw_exe, cli_exe = release / f"gateway{EXE}", release / f"pygg-cli{EXE}"
    if "build" in want:
        code, out, err, secs = run(["cargo", "build", "--release", "--no-default-features", "--bin", "gateway",
                                    "--bin", "pygg-cli"], cwd=TAURI, log=logdir / "build.log")
        R.add("release_build", "Install", code == 0, {"seconds": secs}, "" if code == 0 else err[-300:])

    needs_gateway = {"gateway", "restart", "auto", "setup", "ollama"} & set(want)
    if needs_gateway:
        # Windows lets a second server share a port, so a process left from an earlier run would take requests.
        busy = [p for p in (FAKE_PORT, GW_PORT) if wait_port(p, 0.3)]
        if busy:
            sys.exit(f"ports {busy} are in use (a run left behind?); stop that process first")
    fake = gw = None
    vault_before = vault_entries()
    try:
        if needs_gateway:
            fake = Fake(logdir)
            for old in work.glob("gateway-*"):
                shutil.rmtree(old, ignore_errors=True)
            gw = Gateway(gw_exe, work / f"gateway-{stamp}")
            gw.setup()
            gw.start()
            if "gateway" in want:
                gateway_checks(R, gw, fake)
            if "restart" in want:
                restart_checks(R, gw, fake)
            if "auto" in want:
                auto_checks(R, gw)
            if "ollama" in want:
                ollama_checks(R, gw)
            if "setup" in want:
                setup_checks(R, cli_exe, gw, work)
    finally:
        # Each step on its own: a failure in one must not leave the fake provider, the gateway or the keys behind.
        cleanup = []
        if gw:
            cleanup += [gw.stop, gw.teardown]
            cleanup += [lambda n=n: (gw.work / n).exists() and shutil.copy(gw.work / n, logdir / n)
                        for n in ("gateway.log", "gateway-restarted.log")]
        if fake:
            cleanup.append(fake.stop)
        for step in cleanup:
            try:
                step()
            except Exception as e:  # noqa: BLE001
                print(f"cleanup: {type(e).__name__}: {e}", file=sys.stderr)
    if needs_gateway and vault_before is not None:
        left = vault_entries() - vault_before
        R.add("no_credentials_left", "Install", left == 0, {"entries_left": left})

    if "overhead" in want:
        overhead_checks(R, logdir)
    if "rust" in want:
        rust_checks(R, logdir)
    if "ui" in want:
        ui_checks(R, logdir)
    if "npm" in want:
        npm_checks(R, work, logdir)

    items, metrics = open_items()
    trend = HERE / "trend.jsonl"
    previous = None
    if trend.exists():
        lines = [l for l in trend.read_text(encoding="utf-8").splitlines() if l.strip()]
        previous = json.loads(lines[-1]) if lines else None
    meta = {"time": stamp, "commit": commit, "dirty": dirty, "seconds": round(time.time() - started), "groups": want}
    md, gates = scorecard(meta, R.checks, items, metrics, previous)
    (logdir / "scorecard.md").write_text(md, encoding="utf-8")
    (logdir / "results.json").write_text(json.dumps({**meta, "gates": gates, "open": items, "metrics": metrics,
                                                     "checks": R.checks}, indent=2), encoding="utf-8")
    numbers = {c["id"]: c["numbers"] for c in R.checks if c["numbers"]}
    entry = {**meta, "gates": gates, "passed": sum(c["ok"] is True for c in R.checks),
             "failed": sum(c["ok"] is False for c in R.checks), "skipped": sum(c["ok"] is None for c in R.checks),
             "metrics": metrics, "checks_list": [{"id": c["id"], "ok": c["ok"]} for c in R.checks],
             "numbers": numbers}
    if not args.no_trend and not args.only and not args.skip:
        with trend.open("a", encoding="utf-8", newline="\n") as fh:
            fh.write(json.dumps(entry, separators=(",", ":")) + "\n")
        (HERE / "SCORECARD.md").write_text(md, encoding="utf-8", newline="\n")
    print("\n" + md)
    print(f"written: {logdir}")
    return 1 if entry["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
