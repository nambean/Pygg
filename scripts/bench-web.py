"""Benchmark of Pygg's web-session connections (ADR-035): short tasks sent through the running Pygg gateway, at the
pace Pygg keeps for a web session, so the benchmark itself is never the kind of burst that gets accounts flagged.

    $env:PYGG_BENCH_KEY must hold a Pygg API key (Endpoint -> API Keys); it is read here and never printed.
    python -I scripts/bench-web.py --models muse/muse,chatgpt/chatgpt,... [--rounds 1] [--out DIR] [--base URL]

Per model, the tasks run one after another (Pygg allows one request at a time per account and rests 10 s after each
reply); different sites run side by side, since they are different accounts on different sites. Each request is
streamed, so the time to the first text and the whole time are both measured. A check per task says whether the
answer is right. Results: one JSON line per request in DIR/results.jsonl and a Markdown summary in DIR/summary.md.
Nothing here costs money: web sessions are subscription or free plans, recorded by Pygg as unknown cost.
"""
import argparse, concurrent.futures, datetime, json, os, re, statistics, sys, time, urllib.error, urllib.request

TASKS = [
    ("ok", "Reply with exactly: ok", lambda a: a.strip().strip('.').lower() == "ok"),
    ("arithmetic", "What is 17 * 23? Reply with the number only.", lambda a: "391" in a),
    ("time", "A train leaves at 09:15 and travels for 2 hours 45 minutes. At what time does it arrive? Answer as HH:MM only.",
     lambda a: "12:00" in a),
    ("json", 'Return only this JSON object, unchanged, with no code fence: {"city": "Hanoi", "country": "Vietnam"}',
     lambda a: _json_ok(a)),
    ("code", "Write a Python function is_palindrome(s) that ignores case and non-letters. Reply with the code only.",
     lambda a: "def is_palindrome" in a),
    ("translate", "Translate to English, reply with the translation only: Xin chào thế giới",
     lambda a: "hello" in a.lower() and "world" in a.lower()),
    ("summary", "In one sentence: what does a load balancer do?", lambda a: len(a.strip()) > 20 and "server" in a.lower()),
    ("list", "Name the three primary colors of light, comma-separated, nothing else.",
     lambda a: all(c in a.lower() for c in ("red", "green", "blue"))),
]


def _json_ok(a):
    m = re.search(r"\{.*\}", a, re.S)
    try:
        return m is not None and json.loads(m.group(0)) == {"city": "Hanoi", "country": "Vietnam"}
    except ValueError:
        return False


def one(base, key, model, task):
    name, prompt, check = task
    body = json.dumps({"model": model, "stream": True, "messages": [{"role": "user", "content": prompt}]}).encode()
    req = urllib.request.Request(f"{base}/v1/chat/completions", data=body, method="POST",
                                 headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    started = time.monotonic()
    first, text, status, error = None, "", None, None
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            status = r.status
            for raw in r:
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:") or line == "data: [DONE]":
                    continue
                try:
                    delta = json.loads(line[5:])["choices"][0]["delta"].get("content") or ""
                except (ValueError, KeyError, IndexError):
                    continue
                if delta and first is None:
                    first = time.monotonic() - started
                text += delta
    except urllib.error.HTTPError as e:
        status = e.code
        try:
            err = json.loads(e.read())["error"]
            error = err.get("code") or err.get("type")
        except Exception:
            error = f"HTTP {e.code}"
    except Exception as e:  # network, timeout
        error = type(e).__name__
    total = time.monotonic() - started
    ok = status == 200 and bool(text) and check(text)
    return {"at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"), "model": model, "task": name,
            "status": status, "error": error, "first_s": round(first, 2) if first is not None else None,
            "total_s": round(total, 2), "chars": len(text), "correct": ok}


def run_model(base, key, model, rounds, out):
    rows = []
    for _ in range(rounds):
        for task in TASKS:
            row = one(base, key, model, task)
            rows.append(row)
            with open(out, "a", encoding="utf-8") as f:
                f.write(json.dumps(row) + "\n")
            print(f"{model:24} {row['task']:10} {row['status']} {'ok ' if row['correct'] else 'BAD'} "
                  f"first {row['first_s']}s total {row['total_s']}s {row['error'] or ''}", flush=True)
            if row["status"] == 429 and row["error"] == "daily_limit":
                return rows  # the account's own limit: stop, never push past it
    return rows


def summary(rows):
    lines = ["| Site (model) | Requests | Answered | Correct | Median first text | Median whole answer | Failures |",
             "| --- | --- | --- | --- | --- | --- | --- |"]
    for model in sorted({r["model"] for r in rows}):
        rs = [r for r in rows if r["model"] == model]
        answered = [r for r in rs if r["status"] == 200 and r["chars"]]
        firsts = [r["first_s"] for r in answered if r["first_s"] is not None]
        totals = [r["total_s"] for r in answered]
        fails = {}
        for r in rs:
            if not (r["status"] == 200 and r["chars"]):
                k = r["error"] or f"HTTP {r['status']}"
                fails[k] = fails.get(k, 0) + 1
        lines.append(f"| {model} | {len(rs)} | {len(answered)} | {sum(r['correct'] for r in rs)} | "
                     f"{statistics.median(firsts):.1f} s | {statistics.median(totals):.1f} s | "
                     f"{', '.join(f'{k} x{v}' for k, v in fails.items()) or 'none'} |" if answered else
                     f"| {model} | {len(rs)} | 0 | 0 | - | - | {', '.join(f'{k} x{v}' for k, v in fails.items())} |")
    return "\n".join(lines)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--models", required=True)
    p.add_argument("--rounds", type=int, default=1)
    p.add_argument("--base", default="http://127.0.0.1:8741")
    p.add_argument("--out", default="benchmark/results/web-sessions")
    a = p.parse_args()
    key = os.environ.get("PYGG_BENCH_KEY")
    if not key:
        sys.exit("Set PYGG_BENCH_KEY to a Pygg API key (Endpoint -> API Keys) first.")
    os.makedirs(a.out, exist_ok=True)
    out = os.path.join(a.out, "results.jsonl")
    models = [m.strip() for m in a.models.split(",") if m.strip()]
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(models)) as pool:
        rows = [r for rs in pool.map(lambda m: run_model(a.base, key, m, a.rounds, out), models) for r in rs]
    table = summary(rows)
    with open(os.path.join(a.out, "summary.md"), "w", encoding="utf-8") as f:
        f.write(table + "\n")
    print("\n" + table)


if __name__ == "__main__":
    main()
