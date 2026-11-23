"""ROADMAP focus item 3's proof: what Pygg suggests as checks for real repositories with thin tests.

Input: the JSON lines `detect_proof` prints (checks.rs), one per repository: each suggestion with what it covers.
For each repository it compares with the detection before item 3 (package.json scripts and Cargo only), says whether
every suggestion names what it covers, and, where a parse-only smoke check was suggested (`python -m compileall`),
runs it on a copy twice: as cloned, and with a syntax error added to one file. Nothing else is run: no install, no
test, no build, so no code of the repository executes.

usage: python -I scripts/proof-check-suggestions.py DETECT.jsonl OUT.jsonl
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PYTHON = "python" if os.name == "nt" else "python3"
UNKNOWN = "does not know"


def old_detect(repo: Path):
    """The rules before item 3: package.json typecheck/lint/test/build scripts (with an install first) and Cargo."""
    found = []
    try:
        scripts = json.loads((repo / "package.json").read_text(encoding="utf-8")).get("scripts") or {}
    except (OSError, ValueError):
        scripts = {}
    names = [n for n in ("typecheck", "lint", "test", "build")
             if isinstance(scripts.get(n), str) and "no test specified" not in scripts[n]]
    if names:
        found += ["install"] + names
    if (repo / "Cargo.toml").exists():
        found += ["cargo check", "cargo test"]
    return found


def runs_tests(commands):
    return any(c == "test" or c.endswith(" test") or "test ./..." in c or "pytest" in c or "unittest" in c
               or c == "node --test" for c in commands)


def compileall(repo: Path, broken: bool):
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "repo"
        shutil.copytree(repo, copy, ignore=shutil.ignore_patterns(".git"))
        if broken:
            target = next(p for p in sorted(copy.rglob("*.py")) if ".git" not in p.parts)
            with open(target, "a", encoding="utf-8") as f:
                f.write("\ndef pygg_seeded_break(:\n")
        done = subprocess.run([PYTHON, "-m", "compileall", "-q", "."], cwd=copy, capture_output=True, text=True)
        return done.returncode == 0


def main():
    rows = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8") if l.strip()]
    out = []
    for r in rows:
        repo = Path(r["dir"])
        new = [c["command"] for c in r["checks"]]
        old = old_detect(repo)
        row = {
            "repo": repo.name.replace("_", "/", 1),
            "before": old,
            "after": new,
            "thinBefore": not runs_tests(old),
            "coversNamed": all(UNKNOWN not in c["covers"] for c in r["checks"]) and bool(r["checks"]),
            "smokeOnly": bool(new) and not runs_tests(new),
        }
        if any("compileall" in c for c in new):
            row["compileallClean"] = compileall(repo, broken=False)
            row["compileallCaughtBreak"] = not compileall(repo, broken=True)
        out.append(row)
    with open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
        f.writelines(json.dumps(r) + "\n" for r in out)
    thin = [r for r in out if r["thinBefore"]]
    print(f"{len(out)} repositories; {len(thin)} had no test check before item 3")
    print(f"  some check suggested: before {sum(bool(r['before']) for r in out)}, after {sum(bool(r['after']) for r in out)}")
    print(f"  of the thin ones, a check now: {sum(bool(r['after']) for r in thin)}/{len(thin)}; "
          f"a test-running one: {sum(runs_tests(r['after']) for r in thin)}/{len(thin)}")
    print(f"  every suggestion names what it covers: {sum(r['coversNamed'] for r in out)}/{len(out)}")
    smoke = [r for r in out if "compileallCaughtBreak" in r]
    print(f"  compileall: clean passed {sum(r['compileallClean'] for r in smoke)}/{len(smoke)}, "
          f"seeded break caught {sum(r['compileallCaughtBreak'] for r in smoke)}/{len(smoke)}")
    for r in out:
        print(f"  {r['repo']:40} before {len(r['before'])}  after {len(r['after'])}  "
              f"{'thin' if r['thinBefore'] else '    '}  {', '.join(r['after'])}")


if __name__ == "__main__":
    main()
