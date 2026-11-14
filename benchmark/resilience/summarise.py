"""Markdown tables from the resilience benchmark's results (run.py's *.summary.jsonl).

    python -I benchmark/resilience/summarise.py benchmark/results/2026-11-14-resilience [config ...]
"""

import gzip
import json
import sys
from pathlib import Path


def lines(path):
    """A results file's lines, from `path` or its gzipped copy `path.gz`."""
    path = Path(path)
    if path.exists():
        return path.read_text().splitlines()
    gz = path.with_name(path.name + ".gz")
    if gz.exists():
        return gzip.decompress(gz.read_bytes()).decode().splitlines()
    return None

ORDER = ["healthy", "429-retry-after", "429-no-reset", "402-no-credit", "429-insufficient-quota", "401-revoked",
         "500-down", "503-flaky-30", "unreachable", "dropped", "cut-midstream", "hang-45s", "ring-mixed",
         "ring-all-dead", "ring-affinity", "auto-429-hold"]


def load(folder, config):
    rows = {}
    for line in lines(Path(folder) / f"{config}.summary.jsonl") or []:
        s = json.loads(line)
        if "error" in s:
            continue
        rows.setdefault(s["scenario"], []).append(s)
    return rows


def cell(runs):
    if not runs:
        return "—"
    served = sum(r["served"] for r in runs)
    total = sum(r["requests"] for r in runs)
    att = sum(r["upstream_attempts"] for r in runs) / total
    dup = sum(r["duplicated"] for r in runs)
    text = f"{served}/{total}"
    if served != total:
        text = f"**{text}**"
    text += f" · {att:.2f}"
    if dup:
        text += f" · {dup} dup"
    return text


def latency(runs):
    vals = [r["latency_ms_served"]["p50"] for r in runs if r["latency_ms_served"]["p50"] is not None]
    if not vals:
        return "—"
    return f"{sorted(vals)[len(vals) // 2]:.0f}"


def main():
    folder = sys.argv[1]
    configs = sys.argv[2:] or ["direct", "pygg", "pygg-retry1", "litellm", "litellm-tuned"]
    data = {c: load(folder, c) for c in configs}
    print("Served / sent · upstream attempts per request · duplicated (all runs together)\n")
    print("| Scenario | " + " | ".join(configs) + " |")
    print("| --- | " + " | ".join("---" for _ in configs) + " |")
    for sc in ORDER:
        print(f"| {sc} | " + " | ".join(cell(data[c].get(sc, [])) for c in configs) + " |")
    print("\nMedian latency of served requests, ms (median of the runs' p50)\n")
    print("| Scenario | " + " | ".join(configs) + " |")
    print("| --- | " + " | ".join("---:" for _ in configs) + " |")
    for sc in ORDER:
        print(f"| {sc} | " + " | ".join(latency(data[c].get(sc, [])) for c in configs) + " |")
    for c in configs:
        rows = [r for runs in data[c].values() for r in runs if "ledger" in r]
        if not rows:
            continue
        print(f"\nLedger ({c}): rows against attempts that reached the fake provider\n")
        print("| Scenario | Runs | Upstream attempts | Ledger rows | Rows for attempts that never connected | Requests with rows | Cost unknown | Keys named |")
        print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        for sc in ORDER:
            runs = [r for r in data[c].get(sc, []) if "ledger" in r]
            if not runs:
                continue
            up = sum(r["ledger"]["upstream_attempts_seen_by_fake"] for r in runs)
            rows_n = sum(r["ledger"]["rows"] for r in runs)
            skipped = sum(v for r in runs for k, v in r["ledger"]["outcomes"].items() if k == "fallback:None")
            req = sum(r["ledger"]["requests_with_rows"] for r in runs)
            total = sum(r["requests"] for r in runs)
            print(f"| {sc} | {len(runs)} | {up} | {rows_n} | {skipped} | {req}/{total} | "
                  f"{sum(r['ledger']['cost_unknown_rows'] for r in runs)} | {max(r['ledger']['keys_named'] for r in runs)} |")
    # Requests a provider may have billed twice: served after an attempt that reached a provider and may have run
    # (a 5xx, a connection closed after the request was sent, a stream cut), or answered successfully twice.
    print("\nServed requests that also had an attempt the provider may have run and billed (5xx, dropped, cut, or abandoned after a timeout while the provider ran it)\n")
    print("| Scenario | " + " | ".join(configs) + " |")
    print("| --- | " + " | ".join("---:" for _ in configs) + " |")
    # "?": the gateway gave up on an attempt (a timeout) while the provider was still running it.
    risky = {"http_500", "http_502", "http_503", "http_504", "dropped", "cut_midstream", "?"}
    served_by = {}
    attempts_by = {}
    for c in configs:
        req = lines(Path(folder) / f"{c}.requests.jsonl")
        up = lines(Path(folder) / f"{c}.upstream.jsonl")
        if req is None or up is None:
            continue
        for line in req:
            r = json.loads(line)
            if r["served"]:
                served_by.setdefault((c, r["scenario"]), set()).add((r["run"], r["rid"]))
        for line in up:
            u = json.loads(line)
            if u.get("outcome") in risky and u.get("rid"):
                attempts_by.setdefault((c, u["scenario"]), set()).add((u["run"], u["rid"]))
    for sc in ORDER:
        cells = []
        for c in configs:
            s = served_by.get((c, sc), set())
            cells.append(str(len(s & attempts_by.get((c, sc), set()))) if s else "—")
        print(f"| {sc} | " + " | ".join(cells) + " |")
    for c in configs:
        aff = data[c].get("ring-affinity", [])
        if aff and "sessions" in aff[0]:
            on_one = sum(r["sessions_on_one_key"] for r in aff)
            sessions = sum(r["sessions"] for r in aff)
            print(f"\nSession affinity ({c}): {on_one}/{sessions} sessions stayed on one key; keys used: "
                  f"{sorted({k for r in aff for k in r['keys_used']})}")


if __name__ == "__main__":
    main()
