"""How far an attempt passes its cap when the gateway refuses its next request once the cap is reached (the attempt's
session limit), replayed on real ledgers (ROADMAP gap item 1).

Each session in the ledgers is one benchmark attempt (`x-pygg-session` was the attempt's id). For each paid session
with at least two priced requests, the cap is set to a share of what the session really cost, so that it binds; the
requests are replayed in order, each sent while the counted spend is under the cap. Requests that overlapped in time
(an agent's subagents) are all sent, as the gateway would. Reported: spend when stopped, over the cap, in percent.

usage: python scripts/proof-cap-requests.py STORE [STORE ...] [--share 0.5]
"""
import sqlite3
import sys

share = float(sys.argv[sys.argv.index("--share") + 1]) if "--share" in sys.argv else 0.5
stores = [a for a in sys.argv[1:] if not a.startswith("--") and a != str(share)]
over = []
sessions = 0
for store in stores:
    c = sqlite3.connect(f"file:{store}?mode=ro", uri=True)
    rows = c.execute(
        "SELECT session_key, created_at, created_at - COALESCE(latency_ms, 0), cost_micro_usd FROM usage_events "
        "WHERE session_key IS NOT NULL AND billing = 'paid' AND cost_status IN ('actual', 'estimated') "
        "ORDER BY session_key, created_at").fetchall()
    by = {}
    for key, finished, started, cost in rows:
        by.setdefault(key, []).append((started, finished, cost or 0))
    for key, reqs in by.items():
        total = sum(r[2] for r in reqs)
        if len(reqs) < 2 or total <= 0:
            continue
        sessions += 1
        cap = total * share
        # Counted spend at a request's start: the requests that finished before it started.
        reqs.sort(key=lambda r: r[0])
        spent_when_stopped = 0
        for started, _, cost in reqs:
            counted = sum(c for s, f, c in reqs if f <= started)
            if counted >= cap:
                break
            spent_when_stopped += cost
        over.append(max(0.0, spent_when_stopped - cap) / cap * 100)


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round((len(xs) - 1) * p)))] if xs else None


print(f"sessions {sessions}, cap = {share:.0%} of each session's real cost")
print(f"over the cap: median {pct(over, 0.5):.1f}%, p90 {pct(over, 0.9):.1f}%, max {max(over):.1f}%; "
      f"without the gateway's refusal every one would end {1 / share * 100 - 100:.0f}% over")
