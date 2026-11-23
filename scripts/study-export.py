"""A study participant's figures from their Pygg store, without their code (benchmark/study/README.md, step 4).

Written: for each session `pygg run` launched and each Smart Task in the window, its times, cost (known, unknown,
on a plan), requests, fallbacks, models (counted, not named), attempts, checks (how many ran and passed) and how the
result ended; and per workspace, how many checks Pygg suggested and how many were approved. Not written: prompts,
titles, paths, branch and file names, model names, diffs. Task ids are replaced by a hash.

The file is meant to be read before it is sent. The console also prints, for the person only, each task's reference
next to its title, to fill in the log; that table is not saved.

usage: python -I scripts/study-export.py STORE.sqlite3 --since 2026-12-01 [--until 2026-12-15] --out pygg-export.json
"""
import argparse
import hashlib
import json
import sys
import sqlite3
from datetime import datetime, timezone


def ms(day):
    return int(datetime.strptime(day, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() * 1000)


def ref(value, salt):
    return hashlib.sha256(f"{salt}:{value}".encode()).hexdigest()[:10]


def spend(db, sessions):
    """Money and requests of a set of gateway sessions."""
    if not sessions:
        return {"requests": 0, "knownUsd": 0.0, "unknownCost": 0, "onPlan": 0, "fallbacks": 0, "models": 0}
    marks = ",".join("?" * len(sessions))
    rows = db.execute(
        f"SELECT outcome, cost_micro_usd, cost_status, billing, model FROM usage_events "
        f"WHERE session_key IN ({marks}) AND created_at >= 0", list(sessions)).fetchall()
    known = sum(r[1] or 0 for r in rows if r[2] != "unknown")
    return {
        "requests": len(rows),
        "knownUsd": round(known / 1e6, 4),
        "unknownCost": sum(1 for r in rows if r[2] == "unknown" and r[3] != "subscription"),
        "onPlan": sum(1 for r in rows if r[3] == "subscription"),
        "fallbacks": sum(1 for r in rows if r[0] == "fallback"),
        "models": len({r[4] for r in rows if r[0] == "ok"}),
    }


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    args = argparse.ArgumentParser()
    args.add_argument("store")
    args.add_argument("--since", required=True)
    args.add_argument("--until")
    args.add_argument("--out", required=True)
    a = args.parse_args()
    since, until = ms(a.since), ms(a.until) + 86_400_000 if a.until else 2**62
    db = sqlite3.connect(f"file:{a.store}?mode=ro", uri=True)
    salt = hashlib.sha256(a.store.encode()).hexdigest()[:8]
    out = {"since": a.since, "until": a.until, "launches": [], "tasks": [], "workspaces": []}
    local = []

    has_launches = db.execute("SELECT 1 FROM sqlite_master WHERE name = 'launches'").fetchone()
    for (session, tool, started, finished, exit_code, files, ins, dels, commits) in (db.execute(
            "SELECT session_key, tool, started_at, finished_at, exit_code, files_changed, insertions, deletions, commits "
            "FROM launches WHERE started_at BETWEEN ? AND ? ORDER BY started_at", (since, until)) if has_launches else []):
        out["launches"].append({
            "ref": ref(session, salt), "tool": tool,
            "minutes": round(((finished or started) - started) / 60000, 1),
            "exitCode": exit_code, "filesChanged": files, "insertions": ins, "deletions": dels, "commits": commits,
            **spend(db, [session]),
        })

    for (task_id, title, state, result, created, updated, workspace) in db.execute(
            "SELECT id, title, state, result, created_at, updated_at, workspace_id FROM tasks "
            "WHERE created_at BETWEEN ? AND ? ORDER BY created_at", (since, until)):
        attempts = db.execute(
            "SELECT a.id, a.state, a.billing, a.check_results_json, a.agent_estimate_micro_usd FROM attempts a "
            "JOIN task_stages s ON s.id = a.stage_id WHERE s.task_id = ? ORDER BY a.number", (task_id,)).fetchall()
        checks = [c for a_ in attempts for c in json.loads(a_[3] or "[]")]
        kept = db.execute(
            "SELECT metadata_json FROM audit_events WHERE action = 'task.kept' AND entity = ? ORDER BY created_at DESC",
            (task_id,)).fetchone()
        unchecked = bool(kept and json.loads(kept[0] or "{}").get("unchecked"))
        r = ref(task_id, salt)
        out["tasks"].append({
            "ref": r, "workspace": ref(workspace, salt), "state": state, "result": result,
            "keptUnchecked": unchecked, "minutes": round((updated - created) / 60000, 1),
            "attempts": len(attempts), "paidSteps": sum(1 for x in attempts if x[2] == "paid"),
            "checksRan": len(checks), "checksPassed": sum(1 for c in checks if c.get("passed")),
            "checkedAfterTheStep": db.execute(
                "SELECT COUNT(*) FROM audit_events WHERE action = 'task.checked' AND entity = ?", (task_id,)).fetchone()[0] > 0,
            "agentEstimateUsd": round(sum(x[4] or 0 for x in attempts) / 1e6, 4),
            **spend(db, [x[0] for x in attempts]),
        })
        local.append((r, (title or "")[:60]))

    for (workspace,) in db.execute("SELECT id FROM workspaces"):
        rows = db.execute("SELECT source, approved FROM workspace_checks WHERE workspace_id = ?", (workspace,)).fetchall()
        approvals = db.execute(
            "SELECT COUNT(*) FROM audit_events WHERE action = 'check.approved' AND entity = ? AND created_at BETWEEN ? AND ?",
            (workspace, since, until)).fetchone()[0]
        out["workspaces"].append({
            "ref": ref(workspace, salt),
            "suggested": sum(1 for s, _ in rows if s != "user"),
            "suggestedApproved": sum(1 for s, ok in rows if s != "user" and ok),
            "addedByHand": sum(1 for s, _ in rows if s == "user"),
            "approvalsInWindow": approvals,
        })

    with open(a.out, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(f"wrote {a.out}: {len(out['launches'])} sessions, {len(out['tasks'])} tasks, {len(out['workspaces'])} workspaces")
    print("For your log only (not saved): task reference -> title")
    for r, title in local:
        print(f"  {r}  {title}")


if __name__ == "__main__":
    main()
