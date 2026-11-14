"""Resilience benchmark: the same fault scenarios through each gateway, against benchmark/resilience/fake_provider.py.

    python -I benchmark/resilience/run.py --gateway pygg --out benchmark/results/2026-11-15-resilience [--only NAME]

Each scenario sets what the fake providers do, (re)starts the gateway on a fresh state so no cooldown carries over,
sends N requests (half streamed) with a unique request id in each, then reads the fake's log of what reached the
providers. A request counts as served when the client got a 2xx answer carrying its own request id and, when
streamed, a complete stream. "Duplicated" is a request the providers answered successfully more than once (billed
twice); "lost" is a request that was not served although a healthy target was configured for it.

Gateways: `direct` (no gateway: the client calls provider `a`), `pygg` (the headless gateway built from this
repository), `litellm` (LiteLLM proxy in Docker, config in litellm-config.yaml). Standard library only.
"""

import argparse
import http.client
import json
import os
import shutil
import socket
import sqlite3
import statistics
import subprocess
import sys
import threading
import time
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
FAKE = "http://127.0.0.1:9900"

# name, what it shows, fake rules, model alias, requests, concurrency, whether a healthy target exists, extra
SCENARIOS = [
    dict(name="healthy", what="both targets healthy", rules={}, alias="bench"),
    dict(name="429-retry-after", what="primary answers 429 with Retry-After: 30", alias="bench",
         rules={"a": {"mode": "429", "retry_after": 30}}),
    dict(name="429-no-reset", what="primary answers 429 with no reset stated", alias="bench",
         rules={"a": {"mode": "429"}}),
    dict(name="402-no-credit", what="primary answers 402 (out of credit)", alias="bench", rules={"a": {"mode": "402"}}),
    dict(name="429-insufficient-quota", what="primary answers 429 insufficient_quota (OpenAI's out-of-credit)",
         alias="bench", rules={"a": {"mode": "quota"}}),
    dict(name="401-revoked", what="primary's key revoked (401)", alias="bench", rules={"a": {"mode": "401"}}),
    dict(name="500-down", what="primary answers 500 to everything", alias="bench", rules={"a": {"mode": "500"}}),
    dict(name="503-flaky-30", what="primary fails 30% of requests with 500", alias="bench",
         rules={"a": {"mode": "flaky", "p": 0.3}}),
    dict(name="unreachable", what="primary's port refuses connections", alias="down", rules={}),
    dict(name="dropped", what="primary reads the request and closes the connection without answering", alias="bench",
         rules={"a": {"mode": "drop"}}),
    dict(name="cut-midstream", what="primary starts a streamed answer and cuts it after two chunks", alias="bench",
         rules={"a": {"mode": "midstream"}}, stream_share=1.0),
    dict(name="hang-45s", what="primary waits 45 s before answering (client gives up at 120 s)", alias="bench",
         rules={"a": {"mode": "hang", "seconds": 45}}, n=8, concurrency=8),
    dict(name="ring-mixed", what="key ring of three accounts: key 1 429, key 2 402, key 3 healthy", alias="ring",
         rules={"r:key-r1": {"mode": "429", "retry_after": 30}, "r:key-r2": {"mode": "402"}, "r": {"mode": "ok"}}),
    dict(name="ring-all-dead", what="key ring of three accounts all failing (429, 402, 401): fall back to b",
         alias="ring", rules={"r:key-r1": {"mode": "429", "retry_after": 30}, "r:key-r2": {"mode": "402"},
                              "r:key-r3": {"mode": "401"}}),
    dict(name="ring-affinity", what="key ring of three healthy accounts: does a session stay on one key?",
         alias="ring", rules={}, session_size=5),
    # Pygg only: `auto/cheap` chooses the cheapest eligible model (a) and the next one (b); a 429 with no reset holds
    # a out of auto for 10 minutes. Other gateways have no such alias.
    dict(name="auto-429-hold", what="auto/cheap: the cheapest model answers 429 with no reset", alias="auto/cheap",
         rules={"a": {"mode": "429"}}, gateways=["pygg"], prices={"a": ("0.5", "1"), "b": ("1", "2"),
                                                                   "d": ("5", "10"), "r": ("5", "10")}),
]


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


def pct(values, p):
    if not values:
        return None
    values = sorted(values)
    k = (len(values) - 1) * p
    lo = int(k)
    hi = min(lo + 1, len(values) - 1)
    return round(values[lo] + (values[hi] - values[lo]) * (k - lo), 1)


# ---------------------------------------------------------------- gateways

class Direct:
    name = "direct"
    port = 9900

    def __init__(self, args):
        self.args = args

    def start(self, scenario):
        return True

    def stop(self):
        pass

    def route(self, alias):
        provider = {"bench": "a", "ring": "r", "down": "d"}[alias]
        key = {"a": "key-a1", "r": "key-r1", "d": "key-d1"}[provider]
        port = 9901 if provider == "d" else 9900
        return port, f"/{provider}/v1/chat/completions", key, "fake-model"

    def headers(self, session):
        return {}

    def ledger(self):
        return None

    def version(self):
        return "none"


class Pygg:
    """The headless gateway (`gateway serve`) on a copy of a store set up once by `setup()`."""
    name = "pygg"
    port = 8790

    def __init__(self, args):
        self.args = args
        self.exe = Path(args.pygg_exe)
        self.work = Path(args.work) / "pygg"
        # One template per fake provider port, since the connections point at it.
        self.tdir = self.work / ("template" if args.fake_port == 9900 else f"template-{args.fake_port}")
        self.template = self.tdir / "store.db"
        self.proc = None
        self.db = None
        self.token = None
        self.ids = {}

    def cli(self, db, *argv, env=None):
        out = subprocess.run([str(self.exe), "--db", str(db), *argv], capture_output=True, text=True,
                             env={**os.environ, **(env or {})}, timeout=120)
        if out.returncode != 0:
            raise RuntimeError(f"gateway {argv[0]} failed: {out.stderr.strip()[:300]}")
        return out.stdout

    def setup(self):
        meta = self.tdir / "meta.json"
        if meta.exists():
            m = json.loads(meta.read_text())
            self.token, self.ids = m["token"], m["ids"]
            return
        self.tdir.mkdir(parents=True, exist_ok=True)
        db = self.template
        env = {f"FAKE_KEY_{k.upper().replace('-', '_')}": k for k in
               ("key-a1", "key-b1", "key-d1", "key-r1", "key-r2", "key-r3")}

        def add(preset, label, path, key):
            out = self.cli(db, "add-connection", "--preset", preset, "--label", label, "--endpoint",
                           f"http://127.0.0.1:{9901 if path == 'd' else self.args.fake_port}/{path}/v1", "--key-env",
                           f"FAKE_KEY_{key.upper().replace('-', '_')}", "--billing", "paid", env=env)
            return out.split("Added connection ", 1)[1].split(" ", 1)[0]

        # Four different provider kinds, so each is its own quota group, as four real providers would be.
        self.ids = {"a": add("deepinfra", "Fake A", "a", "key-a1"), "b": add("together", "Fake B", "b", "key-b1"),
                    "d": add("fireworks", "Fake D (down)", "d", "key-d1"), "r": add("nebius", "Fake R (ring)", "r", "key-r1")}
        self.cli(db, "key", "--id", self.ids["r"], "--label", "Account 2", "--key-env", "FAKE_KEY_KEY_R2",
                 "--separate-account", "yes", env=env)
        self.cli(db, "key", "--id", self.ids["r"], "--label", "Account 3", "--key-env", "FAKE_KEY_KEY_R3",
                 "--separate-account", "yes", "--strategy", "round_robin", env=env)
        for p in self.ids.values():
            self.cli(db, "model", "--model", f"{p}/fake-model", "--tier", "T2", "--context", "128000", "--max-output",
                     "8192", "--tools", "yes", "--price-in", "1", "--price-out", "2", "--price-source", "benchmark")
        self.cli(db, "combo", "--name", "bench", "--member", f"{self.ids['a']}/fake-model", "--member",
                 f"{self.ids['b']}/fake-model")
        self.cli(db, "combo", "--name", "ring", "--member", f"{self.ids['r']}/fake-model", "--member",
                 f"{self.ids['b']}/fake-model")
        self.cli(db, "combo", "--name", "down", "--member", f"{self.ids['d']}/fake-model", "--member",
                 f"{self.ids['b']}/fake-model")
        out = self.cli(db, "create-client", "--name", "bench")
        self.token = out.strip().splitlines()[-1].strip()
        meta.write_text(json.dumps({"token": self.token, "ids": self.ids}))

    def teardown(self):
        # Removes the connections and their keys from the system's credential store.
        if self.template.exists():
            for p in self.ids.values():
                try:
                    self.cli(self.template, "remove-connection", "--id", p)
                except RuntimeError as e:
                    print(e, file=sys.stderr)

    def start(self, scenario):
        run = self.work / "runs" / scenario["name"]
        if run.exists():
            shutil.rmtree(run)
        run.mkdir(parents=True)
        self.db = run / "store.db"
        shutil.copy(self.template, self.db)
        for p, (price_in, price_out) in (scenario.get("prices") or {}).items():
            self.cli(self.db, "model", "--model", f"{self.ids[p]}/fake-model", "--price-in", price_in, "--price-out",
                     price_out, "--price-source", "benchmark")
        if self.args.pygg_paid_retries is not None:
            # Retries after a server error (5xx), off by default: the provider may have run the request.
            self.cli(self.db, "client", "--name", "bench", "--paid-retries", str(self.args.pygg_paid_retries))
        self.log = open(run / "gateway.log", "w")
        self.proc = subprocess.Popen([str(self.exe), "--db", str(self.db), "serve", "--port", str(self.port)],
                                     stdout=self.log, stderr=subprocess.STDOUT)
        return wait_port(self.port, 30)

    def stop(self):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
            self.proc = None
            self.log.close()

    def route(self, alias):
        return self.port, "/v1/chat/completions", self.token, alias

    def headers(self, session):
        return {"x-smarttaskrouter-session": session} if session else {}

    def ledger(self):
        con = sqlite3.connect(f"file:{self.db}?mode=ro", uri=True)
        rows = con.execute("SELECT request_id, provider, model, outcome, http_status, key_id, attempt, cost_status, "
                           "session_key FROM usage_events").fetchall()
        self.holds = [dict(zip(("key", "value"), r)) for r in
                      con.execute("SELECT key, value FROM settings WHERE key LIKE 'auto.hold.%'").fetchall()]
        con.close()
        return [dict(zip(("request_id", "provider", "model", "outcome", "http_status", "key_id", "attempt",
                          "cost_status", "session_key"), r)) for r in rows]

    def version(self):
        return subprocess.run(["git", "-C", str(HERE), "rev-parse", "--short", "HEAD"], capture_output=True,
                              text=True).stdout.strip()


class LiteLLM:
    """LiteLLM proxy in Docker on the benchmark network, restarted for each scenario (its cooldowns live in memory)."""
    name = "litellm"

    def __init__(self, args):
        self.args = args
        self.port = args.litellm_port
        self.container = f"pyggbench-litellm-{self.port}"
        self.image = args.litellm_image
        self.config = Path(args.litellm_config).resolve()

    def setup(self):
        pass

    def teardown(self):
        pass

    def start(self, scenario):
        subprocess.run(["docker", "rm", "-f", self.container], capture_output=True)
        cmd = ["docker", "run", "-d", "--name", self.container, "--network", "pyggbench", "-p",
               f"127.0.0.1:{self.port}:4000", "-v", f"{self.config}:/app/config.yaml:ro", self.image,
               "--config", "/app/config.yaml", "--port", "4000"]
        out = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, "MSYS_NO_PATHCONV": "1"})
        if out.returncode != 0:
            raise RuntimeError(out.stderr)
        deadline = time.time() + 120
        while time.time() < deadline:
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{self.port}/health/liveliness", timeout=2)
                return True
            except Exception:
                time.sleep(1)
        return False

    def stop(self):
        logs = subprocess.run(["docker", "logs", self.container], capture_output=True, text=True, encoding="utf-8", errors="replace")
        self.last_logs = (logs.stdout or "") + (logs.stderr or "")
        subprocess.run(["docker", "rm", "-f", self.container], capture_output=True)

    def route(self, alias):
        return self.port, "/v1/chat/completions", "sk-bench-master", alias

    def headers(self, session):
        # LiteLLM reads a session id from the request's metadata (litellm_session_id) and this header.
        return {"x-litellm-session-id": session} if session else {}

    def ledger(self):
        return None

    def version(self):
        return self.image


GATEWAYS = {"direct": Direct, "pygg": Pygg, "litellm": LiteLLM}


# ---------------------------------------------------------------- client

def one_request(gw, alias, rid, session, stream, timeout):
    port, path, key, model = gw.route(alias)
    body = {"model": model, "messages": [{"role": "user", "content": f"Reply with ok. rid={rid}"}], "max_tokens": 64}
    if stream:
        body["stream"] = True
        body["stream_options"] = {"include_usage": True}
    if session and gw.name == "litellm":
        body["litellm_session_id"] = session
    headers = {"content-type": "application/json", "authorization": f"Bearer {key}", **gw.headers(session)}
    started = time.perf_counter()
    result = {"rid": rid, "session": session, "stream": stream, "status": None, "served": False, "via": None,
              "error": None, "complete": None, "ttfb_ms": None, "ms": None}
    try:
        conn = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
        conn.request("POST", path, json.dumps(body), headers)
        resp = conn.getresponse()
        result["status"] = resp.status
        result["ttfb_ms"] = round((time.perf_counter() - started) * 1000, 1)
        if not stream or resp.status != 200:
            raw = resp.read().decode(errors="replace")
            if resp.status == 200:
                data = json.loads(raw)
                text = data["choices"][0]["message"]["content"] or ""
                result["complete"] = True
            else:
                text = ""
                result["error"] = raw[:300]
        else:
            text, done, finished = "", False, False
            buffer = b""
            try:
                while True:
                    chunk = resp.read1(65536) if hasattr(resp, "read1") else resp.read(1024)
                    if not chunk:
                        break
                    buffer += chunk
                    while b"\n\n" in buffer:
                        event, buffer = buffer.split(b"\n\n", 1)
                        for line in event.split(b"\n"):
                            if not line.startswith(b"data:"):
                                continue
                            payload = line[5:].strip()
                            if payload == b"[DONE]":
                                done = True
                                continue
                            try:
                                obj = json.loads(payload)
                            except ValueError:
                                continue
                            if "error" in obj:
                                result["error"] = json.dumps(obj["error"])[:300]
                            for choice in obj.get("choices") or []:
                                text += (choice.get("delta") or {}).get("content") or ""
                                if choice.get("finish_reason"):
                                    finished = True
            except Exception as e:  # the connection broke mid-stream
                result["error"] = f"stream broke: {type(e).__name__}: {e}"[:300]
            result["complete"] = finished and not (result["error"] or "").startswith("stream broke")
            result["text"] = text[:200]
        if resp.status == 200:
            result["text"] = text[:200]
            ok_rid = f"rid={rid}" in text
            result["via"] = text.split("via=", 1)[1].split()[0] if "via=" in text else None
            result["rid_count"] = text.count("rid=")
            result["served"] = bool(ok_rid and result["complete"] and result["rid_count"] == 1)
        conn.close()
    except Exception as e:
        result["error"] = f"{type(e).__name__}: {e}"[:300]
    result["ms"] = round((time.perf_counter() - started) * 1000, 1)
    return result


def run_scenario(gw, sc, args):
    n = sc.get("n", args.n)
    concurrency = sc.get("concurrency", args.concurrency)
    stream_share = sc.get("stream_share", 0.5)
    session_size = sc.get("session_size", args.session_size)
    http_json("POST", f"{FAKE}/_admin/config", {"latency_ms": args.latency_ms, "rules": sc["rules"]})
    if not gw.start(sc):
        raise RuntimeError(f"{gw.name} did not start")
    time.sleep(0.5)
    http_json("POST", f"{FAKE}/_admin/reset", {})
    tag = uuid.uuid4().hex[:6]
    jobs = []
    for i in range(n):
        rid = f"{sc['name'][:10]}-{tag}-{i:04d}".replace(".", "")
        session = f"s-{tag}-{i // session_size:03d}"
        stream = (i % 100) < stream_share * 100 if stream_share < 1 else True
        if stream_share == 0.5:
            stream = i % 2 == 1
        jobs.append((rid, session, stream))
    t0 = time.time()
    # Sessions are sent in order within themselves: request k+1 of a session starts after request k returned.
    by_session = {}
    for job in jobs:
        by_session.setdefault(job[1], []).append(job)

    def run_session(session_jobs):
        return [one_request(gw, sc["alias"], rid, session, stream, args.timeout) for rid, session, stream in session_jobs]

    results = []
    with ThreadPoolExecutor(concurrency) as pool:
        for chunk in pool.map(run_session, by_session.values()):
            results.extend(chunk)
    wall = time.time() - t0
    time.sleep(1.0)
    upstream = [json.loads(line) for line in http_json("GET", f"{FAKE}/_admin/log").splitlines() if line.strip()]
    gw.stop()
    ledger = gw.ledger()
    summary = summarise(gw, sc, results, upstream, ledger, wall)
    if sc.get("gateways") and getattr(gw, "holds", None) is not None:
        summary["auto_holds"] = gw.holds
    return summary, results, upstream, ledger


def summarise(gw, sc, results, upstream, ledger, wall):
    rids = {r["rid"] for r in results}
    up = [u for u in upstream if u.get("rid") in rids]
    ok_up = {}
    attempts = {}
    for u in up:
        attempts[u["rid"]] = attempts.get(u["rid"], 0) + 1
        if u["outcome"] == "ok":
            ok_up[u["rid"]] = ok_up.get(u["rid"], 0) + 1
    served = [r for r in results if r["served"]]
    lat = [r["ms"] for r in served]
    by_target = {}
    for u in up:
        k = f"{u['provider']}/{u['key']}:{u['outcome']}"
        by_target[k] = by_target.get(k, 0) + 1
    statuses = {}
    for r in results:
        k = str(r["status"]) if r["served"] else f"{r['status']}:{'incomplete' if r['status'] == 200 else 'error'}"
        statuses[k] = statuses.get(k, 0) + 1
    summary = {
        "gateway": gw.name, "scenario": sc["name"], "what": sc["what"], "requests": len(results),
        "served": len(served), "success_rate": round(len(served) / len(results), 4) if results else None,
        "not_served": len(results) - len(served),
        "client_outcomes": statuses,
        "upstream_attempts": len(up), "attempts_per_request": round(len(up) / len(results), 3) if results else None,
        "duplicated": sum(1 for v in ok_up.values() if v > 1),
        "served_without_upstream_ok": sum(1 for r in served if ok_up.get(r["rid"], 0) == 0),
        "upstream_by_target": dict(sorted(by_target.items())),
        "latency_ms_served": {"p50": pct(lat, 0.5), "p95": pct(lat, 0.95), "max": max(lat) if lat else None},
        "ttfb_ms_p50": pct([r["ttfb_ms"] for r in served if r["ttfb_ms"]], 0.5),
        "wall_s": round(wall, 2),
    }
    if sc.get("session_size") or sc["name"] == "ring-affinity":
        sessions = {}
        for r in served:
            sessions.setdefault(r["session"], set()).add(r["via"])
        summary["sessions"] = len(sessions)
        summary["sessions_on_one_key"] = sum(1 for v in sessions.values() if len(v) == 1)
        summary["keys_used"] = sorted({v for s in sessions.values() for v in s if v})
    if ledger is not None:
        reached = [l for l in ledger if l["http_status"] is not None or l["outcome"] in ("ok", "failed", "error")]
        outcomes = {}
        for l in ledger:
            k = f"{l['outcome']}:{l['http_status']}"
            outcomes[k] = outcomes.get(k, 0) + 1
        summary["ledger"] = {
            "rows": len(ledger), "outcomes": dict(sorted(outcomes.items())),
            "requests_with_rows": len({l["request_id"] for l in ledger if l["request_id"]}),
            "ok_rows": sum(1 for l in ledger if l["outcome"] == "ok"),
            "upstream_attempts_seen_by_fake": len(up),
            "rows_vs_upstream": len(ledger) - len(up),
            "cost_unknown_rows": sum(1 for l in ledger if l["cost_status"] == "unknown"),
            "keys_named": len({l["key_id"] for l in ledger if l["key_id"]}),
        }
    return summary


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gateway", required=True, choices=sorted(GATEWAYS))
    ap.add_argument("--out", required=True)
    ap.add_argument("--only", action="append")
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--concurrency", type=int, default=4)
    ap.add_argument("--session-size", type=int, default=5)
    ap.add_argument("--latency-ms", type=int, default=40)
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--repeat", type=int, default=1)
    ap.add_argument("--work", default=os.environ.get("PYGG_BENCH_WORK", str(HERE / ".work")))
    ap.add_argument("--pygg-exe", default=os.environ.get("PYGG_GATEWAY_EXE", "gateway"))
    ap.add_argument("--pygg-paid-retries", type=int, help="set the client's retries after a server error")
    ap.add_argument("--label", help="name of this configuration in the results (default: the gateway's)")
    ap.add_argument("--litellm-port", type=int, default=4000)
    ap.add_argument("--fake-port", type=int, default=9900, help="the fake provider's port on 127.0.0.1")
    ap.add_argument("--litellm-image", default="ghcr.io/berriai/litellm:v1.106.2")  # never a floating tag (2026-10-21 incident)
    ap.add_argument("--litellm-config", default=str(HERE / "litellm-config.yaml"))
    ap.add_argument("--teardown", action="store_true", help="remove the Pygg template's connections and keys, then exit")
    args = ap.parse_args()
    global FAKE
    FAKE = f"http://127.0.0.1:{args.fake_port}"
    gw = GATEWAYS[args.gateway](args)
    label = args.label or gw.name
    if args.teardown:
        gw.teardown()
        return
    if hasattr(gw, "setup"):
        gw.setup()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    scenarios = [s for s in SCENARIOS if (not args.only or s["name"] in args.only)
                 and (not s.get("gateways") or gw.name in s["gateways"])]
    for rep in range(1, args.repeat + 1):
        for sc in scenarios:
            print(f"[{gw.name}] {sc['name']} (run {rep})", flush=True)
            try:
                summary, results, upstream, ledger = run_scenario(gw, sc, args)
            except Exception as e:
                gw.stop()
                print(f"  failed: {e}", flush=True)
                summary, results, upstream, ledger = {"gateway": gw.name, "scenario": sc["name"], "error": str(e)}, [], [], None
            summary["config"] = label
            summary["run"] = rep
            summary["gateway_version"] = gw.version()
            summary["at"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
            print("  " + json.dumps({k: summary.get(k) for k in ("served", "requests", "upstream_attempts", "duplicated",
                                                                    "latency_ms_served")}), flush=True)
            with open(out / f"{label}.summary.jsonl", "a") as f:
                f.write(json.dumps(summary) + "\n")
            with open(out / f"{label}.requests.jsonl", "a") as f:
                for r in results:
                    f.write(json.dumps({"scenario": sc["name"], "run": rep, **r}) + "\n")
            with open(out / f"{label}.upstream.jsonl", "a") as f:
                for u in upstream:
                    f.write(json.dumps({"scenario": sc["name"], "run": rep, **u}) + "\n")
            if ledger is not None:
                with open(out / f"{label}.ledger.jsonl", "a") as f:
                    for l in ledger:
                        f.write(json.dumps({"scenario": sc["name"], "run": rep, **l}) + "\n")


if __name__ == "__main__":
    main()
