"""The model facts feed (ADR-040, docs/design/BRAIN.md section 1).

Builds `model-facts/v1/facts.json` from the public sources the owner approved, each read under its own terms, and
signs it. The app never reads a leaderboard; it fetches this file, verifies the signature and ranks connected models
per role from it. A fact with no source has no row; a name the names map does not know is reported and left out.

    python scripts/model-facts.py build [--out model-facts/v1/facts.json] [--previous facts.json] [--only SOURCE]
    python scripts/model-facts.py sign facts.json --key-id ID            # seed in $MODEL_FACTS_SIGNING_KEY (base64)
    python scripts/model-facts.py verify facts.json [facts.json.sig] --public HEX
    python scripts/model-facts.py keygen                                 # prints a public key; writes the seed to --seed-file
    python scripts/model-facts.py names                                  # names the sources use that the map does not know
    python scripts/model-facts.py selftest                               # the parsers on model-facts/fixtures

Sources (terms read on 2026-11-19; see BRAIN.md for the table):
    terminal-bench        harbor-framework/terminal-bench, leaderboard/submissions/*.json, Apache-2.0        role code
    swe-bench-verified    SWE-bench/swe-bench.github.io, data/leaderboards.json (Verified), CC BY-NC 4.0     role code
    aider-polyglot        Aider-AI/aider, aider/website/_data/polyglot_leaderboard.yml, Apache-2.0          role code
    lmarena-hard-prompts  Hugging Face lmarena-ai/leaderboard-dataset, text_style_control/latest, CC BY 4.0 roles plan, review (derived)
    lmarena-text-to-image the same dataset, text_to_image/latest, CC BY 4.0                                  role image

Needs: pyyaml, pyarrow, cryptography (`pip install pyyaml pyarrow cryptography`).
"""
from __future__ import annotations

import argparse
import base64
import datetime as dt
import io
import json
import os
import re
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FEED_DIR = ROOT / "model-facts"
NAMES_PATH = FEED_DIR / "names.json"
FIXTURES = FEED_DIR / "fixtures"
VERSION = 1
USER_AGENT = "pygg-model-facts/1 (+https://github.com/nambean/Pygg)"
TWELVE_MONTHS = 366

# What each source is: where its data is, under what terms, what it measures and does not, and the roles it feeds.
SOURCES = {
    "terminal-bench": {
        "name": "Terminal-Bench 4.x",
        "url": "https://www.tbench.ai/leaderboard",
        "data": "https://codeload.github.com/harbor-framework/terminal-bench/tar.gz/main",
        "licence": "Apache-2.0",
        "attribution": "Terminal-Bench (Stanford, Laude Institute, Harbor), leaderboard submissions, Apache-2.0",
        "measures": "tasks solved in a terminal by an agent with the model; not your repository",
        "roles": ["code"],
        "derived": {},
        "unit": "% resolved",
    },
    "swe-bench-verified": {
        "name": "SWE-bench Verified",
        "url": "https://www.swebench.com/",
        "data": "https://raw.githubusercontent.com/SWE-bench/swe-bench.github.io/master/data/leaderboards.json",
        "licence": "CC-BY-NC-4.0",
        "attribution": "SWE-bench Verified leaderboard (SWE-bench), CC BY-NC 4.0; non-commercial use only",
        "measures": "real GitHub issues resolved by an agent with the model, judged by tests; not your repository",
        "roles": ["code"],
        "derived": {},
        "unit": "% resolved",
    },
    "aider-polyglot": {
        "name": "Aider polyglot",
        "url": "https://aider.chat/docs/leaderboards/",
        "data": "https://raw.githubusercontent.com/Aider-AI/aider/main/aider/website/_data/polyglot_leaderboard.yml",
        "licence": "Apache-2.0",
        "attribution": "Aider polyglot leaderboard (Aider-AI), Apache-2.0",
        "measures": "Exercism exercises in six languages edited through Aider, judged by tests; not your repository",
        "roles": ["code"],
        "derived": {},
        "unit": "% passed (second try)",
    },
    "lmarena-hard-prompts": {
        "name": "LMArena hard prompts",
        "url": "https://huggingface.co/datasets/lmarena-ai/leaderboard-dataset",
        "data": "https://huggingface.co/datasets/lmarena-ai/leaderboard-dataset/resolve/main/text_style_control/latest-00000-of-00001.parquet",
        "licence": "CC-BY-4.0",
        "attribution": "Arena leaderboard dataset (lmarena-ai), CC BY 4.0",
        "measures": "human preference between two answers to hard prompts, with style control; not software planning",
        "roles": ["plan", "review"],
        "derived": {"plan": True, "review": True},
        "unit": "Arena rating",
        "category": "hard_prompts",
    },
    "lmarena-text-to-image": {
        "name": "LMArena text-to-image",
        "url": "https://huggingface.co/datasets/lmarena-ai/leaderboard-dataset",
        "data": "https://huggingface.co/datasets/lmarena-ai/leaderboard-dataset/resolve/main/text_to_image/latest-00000-of-00001.parquet",
        "licence": "CC-BY-4.0",
        "attribution": "Arena leaderboard dataset (lmarena-ai), CC BY 4.0",
        "measures": "human preference between two images for one prompt",
        "roles": ["image"],
        "derived": {},
        "unit": "Arena rating",
        "category": "overall",
    },
}

EFFORTS = {"low", "medium", "high", "xhigh", "max", "thinking", "none", "minimal"}


# ---------------------------------------------------------------------------------------------------- identity
def canonical(model_id: str) -> str:
    """One spelling for a model id across providers: the tail after a router's vendor prefix, lower-cased, `.`, `_`
    and spaces as `-`, a `:free` or `:nitro` variant, a `-latest` and a trailing date (`-20251101`, `-2025-11-01`)
    dropped. `openrouter/anthropic/claude-opus-4.5` and `claude-opus-4-5-20251101` both become `claude-opus-4-5`."""
    tail = model_id.strip().split("/")[-1].lower()
    tail = re.sub(r":(free|nitro|floor|exacto|online)$", "", tail)
    tail = re.sub(r"[._\s]+", "-", tail)
    tail = re.sub(r"-(latest)$", "", tail)
    tail = re.sub(r"-(\d{8}|\d{4}-\d{2}-\d{2})$", "", tail)
    return re.sub(r"-{2,}", "-", tail).strip("-")


def effort_of(text: str) -> str | None:
    """The reasoning effort a display name states in parentheses: `Claude Opus 5.5 (High)` is `high`."""
    inside = " ".join(re.findall(r"\(([^)]*)\)", text)).lower()
    for token in re.split(r"[,\s]+", inside):
        if token in EFFORTS:
            return token
    return None


def propose_ids(name: str) -> list[str]:
    """A canonical id guessed from a display name, for seeding the names map; always reviewed by hand."""
    base = re.sub(r"\([^)]*\)", "", name).strip().lower()
    base = re.sub(r"\bw/.*$", "", base).strip()
    words = re.split(r"[\s]+", base)
    # SWE-bench writes `Claude 4.5 Opus`; Anthropic's ids are `claude-opus-4-5` from the 4 line on, and
    # `claude-3-5-sonnet` before it, so the order is swapped only for 4 and later.
    if words and words[0] == "claude" and len(words) >= 3 and re.match(r"^\d", words[1]) and words[2] in ("opus", "sonnet", "haiku"):
        if int(words[1].split(".")[0]) >= 4:
            words = [words[0], words[2], words[1]] + words[3:]
    joined = "-".join(words)
    return [canonical(joined)] if joined else []


# ---------------------------------------------------------------------------------------------------- fetching
def fetch(url: str, timeout: int = 120) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as response:  # noqa: S310 (fixed https URLs above)
        return response.read()


# ---------------------------------------------------------------------------------------------------- parsers
# Each parser returns rows: {name, ids, effort, agent, score, ci, date, cost, url}. `ids` are canonical ids when the
# source states a provider id; else None, and the names map must say.
def parse_terminal_bench(data: bytes) -> list[dict]:
    rows = []
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        for member in tar.getmembers():
            if "/leaderboard/submissions/" not in member.name or not member.name.endswith(".json"):
                continue
            entry = json.load(tar.extractfile(member))
            meta, metrics, source = entry.get("metadata", {}), entry.get("metrics", {}), entry.get("source_filter", {})
            if "accuracy" not in metrics or not source.get("model_name"):
                continue
            rows.append({
                "name": meta.get("model_display", {}).get("label") or source["model_name"],
                "ids": [canonical(source["model_name"])],
                "effort": (source.get("reasoning_effort") or meta.get("reasoning_effort") or None),
                "agent": source.get("agent"),
                "score": float(metrics["accuracy"]),
                "ci": metrics.get("accuracy_ci95_half_width"),
                "date": meta.get("date"),
                "cost": metrics.get("total_cost_usd"),
                "url": (entry.get("source_jobs") or [None])[0],
            })
    return rows


def parse_swe_bench(data: bytes) -> list[dict]:
    boards = json.loads(data)["leaderboards"]
    verified = next(b for b in boards if b["name"] == "Verified")["results"]
    rows = []
    for r in verified:
        display = r.get("model_display") or ""
        if not display or display in ("Undisclosed", "Multiple") or r.get("resolved") is None:
            continue
        rows.append({
            "name": display,
            "ids": None,
            "effort": r.get("reasoning_effort") or None,
            "agent": r.get("agent"),
            "score": float(r["resolved"]),
            "ci": None,
            "date": r.get("date"),
            "cost": r.get("cost"),
            "url": r.get("site"),
        })
    return rows


def parse_aider(data: bytes) -> list[dict]:
    import yaml  # type: ignore

    rows = []
    for e in yaml.safe_load(data.decode("utf-8")) or []:
        command = str(e.get("command") or "")
        m = re.search(r"--model\s+(\S+)", command)
        if not m or e.get("pass_rate_2") is None:
            continue
        model = m.group(1).strip('"')
        rows.append({
            "name": str(e.get("model")),
            "ids": [canonical(model)],
            "effort": effort_of(str(e.get("model"))) or (str(e["reasoning_effort"]) if e.get("reasoning_effort") else None),
            "agent": "aider",
            "score": float(e["pass_rate_2"]),
            "ci": None,
            "date": str(e.get("date")) if e.get("date") else None,
            "cost": e.get("total_cost"),
            "url": None,
        })
    return rows


def parse_lmarena(data: bytes, category: str | None) -> list[dict]:
    import pyarrow.parquet as pq  # type: ignore

    return parse_lmarena_rows(pq.read_table(io.BytesIO(data)).to_pylist(), category)


def parse(source_id: str, data: bytes) -> list[dict]:
    if source_id == "terminal-bench":
        return parse_terminal_bench(data)
    if source_id == "swe-bench-verified":
        return parse_swe_bench(data)
    if source_id == "aider-polyglot":
        return parse_aider(data)
    if source_id.startswith("lmarena"):
        return parse_lmarena(data, SOURCES[source_id]["category"])
    raise ValueError(source_id)


# ---------------------------------------------------------------------------------------------------- facts
def load_names() -> dict:
    return json.loads(NAMES_PATH.read_text(encoding="utf-8")) if NAMES_PATH.exists() else {}


def facts_of(source_id: str, rows: list[dict], names: dict, unmapped: dict) -> list[dict]:
    """Rows to facts: the names map resolves names without a provider id; places and ranks are computed over the
    rows kept (one source, one leaderboard), so a dropped row never moves another's place."""
    source = SOURCES[source_id]
    kept = []
    for r in rows:
        ids, effort = r["ids"], r["effort"]
        if ids is None:
            entry = names.get(source_id, {}).get(r["name"])
            if not entry or not entry.get("ids"):
                unmapped.setdefault(source_id, []).append(r["name"])
                continue
            ids = entry["ids"]
            effort = entry.get("effort", effort)
        kept.append({**r, "ids": ids, "effort": effort})
    scores = sorted((k["score"] for k in kept), reverse=True)
    of = len(kept)
    facts = []
    for k in kept:
        place = 1 + sum(1 for s in scores if s > k["score"])
        rank = 1.0 if of <= 1 else round((of - place) / (of - 1), 4)
        for role in source["roles"]:
            fact = {
                "ids": k["ids"],
                "name": k["name"],
                "role": role,
                "score": round(k["score"], 2),
                "unit": source["unit"],
                        "ci": round(k["ci"], 2) if k.get("ci") is not None else None,
                "place": place,
                "of": of,
                "rank": rank,
                "derived": bool(source["derived"].get(role, False)),
                "source": source_id,
                "url": k.get("url") or source["url"],
                "date": k.get("date"),
                "agent": k.get("agent"),
                "effort": k.get("effort"),
                "licence": source["licence"],
            }
            if k.get("votes") is not None:
                fact["votes"] = k["votes"]
            if k.get("cost") is not None:
                fact["cost_usd"] = k["cost"]
            facts.append(fact)
    return facts


def build(out: Path, previous: Path | None, only: str | None, fixtures: Path | None = None) -> dict:
    names = load_names()
    old = json.loads(previous.read_text(encoding="utf-8")) if previous and previous.exists() else None
    today = dt.date.today().isoformat()
    sources_out, facts, unmapped = [], [], {}
    for source_id, source in SOURCES.items():
        if only and source_id != only:
            continue
        entry = {k: source[k] for k in ("name", "url", "licence", "attribution", "measures", "roles")}
        entry.update({"id": source_id, "terms_read": "2026-11-19"})
        try:
            if fixtures is not None:
                rows = parse_fixture(source_id, (fixtures / fixture_name(source_id)).read_bytes())
            else:
                rows = parse(source_id, fetch(source["data"]))
            new = facts_of(source_id, rows, names, unmapped)
            if not new:
                raise ValueError("no rows parsed")
            entry.update({"status": "ok", "observed": today, "rows": len({f['name'] for f in new})})
            facts.extend(new)
        except Exception as e:  # noqa: BLE001 (a failed source is carried forward, never guessed)
            kept = [f for f in (old or {}).get("facts", []) if f.get("source") == source_id] if old else []
            old_entry = next((s for s in (old or {}).get("sources", []) if s.get("id") == source_id), None)
            print(f"{source_id}: failed ({e}); carrying {len(kept)} facts forward", file=sys.stderr)
            entry.update({"status": "stale", "observed": (old_entry or {}).get("observed"), "rows": (old_entry or {}).get("rows"), "error": str(e)[:200]})
            facts.extend(kept)
        sources_out.append(entry)
    if not any(s["status"] == "ok" for s in sources_out):
        raise SystemExit("every source failed; nothing to publish")
    for source_id, items in unmapped.items():
        print(f"{source_id}: {len(items)} names not in the map, left out: {', '.join(sorted(set(items))[:20])}{' …' if len(set(items)) > 20 else ''}", file=sys.stderr)
    feed = {
        "version": VERSION,
        "built": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "sources": sources_out,
        "facts": facts,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(json.dumps(feed, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))
    print(f"wrote {out}: {len(facts)} facts from {sum(1 for s in sources_out if s['status'] == 'ok')} sources", file=sys.stderr)
    return feed


FIXTURE_NAMES = {"terminal-bench": "terminal-bench.tar.gz", "swe-bench-verified": "swe-bench-verified.json",
                 "aider-polyglot": "aider-polyglot.yml", "lmarena-hard-prompts": "lmarena-hard-prompts.json",
                 "lmarena-text-to-image": "lmarena-text-to-image.json"}


def fixture_name(source_id: str) -> str:
    return FIXTURE_NAMES[source_id]


def parse_fixture(source_id: str, data: bytes) -> list[dict]:
    """Fixtures hold a source's rows in its own shape, except LMArena, whose Parquet is kept as JSON rows."""
    if source_id.startswith("lmarena"):
        return parse_lmarena_rows(json.loads(data), SOURCES[source_id]["category"])
    return parse(source_id, data)


def parse_lmarena_rows(table: list[dict], category: str | None) -> list[dict]:
    rows = []
    for r in table:
        if r.get("category") != category or r.get("rating") is None:
            continue
        rows.append({
            "name": r["model_name"], "ids": None, "effort": effort_of(r["model_name"]), "agent": None,
            "score": float(r["rating"]),
            "ci": (float(r["rating_upper"]) - float(r["rating_lower"])) / 2 if r.get("rating_upper") is not None else None,
            "date": str(r.get("leaderboard_publish_date") or "")[:10] or None, "cost": None, "url": None,
            "votes": int(r["vote_count"]) if r.get("vote_count") is not None else None,
        })
    return rows


# ---------------------------------------------------------------------------------------------------- signing
def _keys():
    from cryptography.hazmat.primitives.asymmetric import ed25519  # type: ignore

    return ed25519


def sign(path: Path, key_id: str, seed_b64: str) -> Path:
    ed25519 = _keys()
    seed = base64.b64decode(seed_b64)
    if len(seed) != 32:
        raise SystemExit("the signing seed must be 32 bytes, base64")
    key = ed25519.Ed25519PrivateKey.from_private_bytes(seed)
    signature = key.sign(path.read_bytes())
    sig_path = path.with_name(path.name + ".sig")
    sig_path.write_text(f"{key_id}:{base64.b64encode(signature).decode('ascii')}\n", encoding="ascii")
    return sig_path


def verify(path: Path, sig_path: Path, public_hex: str) -> str:
    ed25519 = _keys()
    key_id, _, sig_b64 = sig_path.read_text(encoding="ascii").strip().partition(":")
    public = ed25519.Ed25519PublicKey.from_public_bytes(bytes.fromhex(public_hex))
    public.verify(base64.b64decode(sig_b64), path.read_bytes())
    return key_id


def keygen(seed_file: Path | None) -> None:
    ed25519 = _keys()
    from cryptography.hazmat.primitives import serialization  # type: ignore

    key = ed25519.Ed25519PrivateKey.generate()
    seed = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw, serialization.NoEncryption())
    public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    if seed_file:
        seed_file.write_text(base64.b64encode(seed).decode("ascii"), encoding="ascii")
        print(f"seed written to {seed_file} (base64, 32 bytes): set it as the repository secret MODEL_FACTS_SIGNING_KEY and delete the file")
    else:
        print("no --seed-file given; the seed was not written anywhere and this key pair is lost")
    print(f"public key (hex, for gateway/facts.rs KEYS): {public.hex()}")


# ---------------------------------------------------------------------------------------------------- names
def names_report() -> int:
    names = load_names()
    missing = 0
    for source_id in ("swe-bench-verified", "lmarena-hard-prompts", "lmarena-text-to-image"):
        try:
            rows = parse(source_id, fetch(SOURCES[source_id]["data"]))
        except Exception as e:  # noqa: BLE001
            print(f"{source_id}: could not read ({e})")
            continue
        known = names.get(source_id, {})
        for r in rows:
            if r["name"] not in known:
                missing += 1
                print(f'{source_id}\t{r["name"]}\t{",".join(propose_ids(r["name"]))}\t{effort_of(r["name"]) or ""}')
    return 1 if missing else 0


# ---------------------------------------------------------------------------------------------------- selftest
def selftest() -> int:
    """Builds from the fixtures and checks the parsers, places, ranks, the map and the carry-forward."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "facts.json"
        feed = build(out, None, None, fixtures=FIXTURES)
        by_source = {}
        for f in feed["facts"]:
            by_source.setdefault(f["source"], []).append(f)
        tb = by_source["terminal-bench"]
        assert all(f["role"] == "code" and f["licence"] == "Apache-2.0" for f in tb), tb
        best = min(tb, key=lambda f: f["place"])
        assert best["place"] == 1 and best["rank"] == 1.0 and best["agent"], best
        assert any(f["ids"] == ["claude-sonnet-5"] for f in tb), [f["ids"] for f in tb]
        swe = by_source["swe-bench-verified"]
        assert all(f["licence"] == "CC-BY-NC-4.0" for f in swe)
        assert any(f["ids"] == ["claude-opus-4-5"] and f["effort"] == "high" for f in swe), [(f["ids"], f["effort"]) for f in swe]
        aider = by_source["aider-polyglot"]
        assert any(f["ids"] == ["o1"] and f["effort"] == "high" for f in aider), [(f["ids"], f["effort"]) for f in aider]
        lm = by_source["lmarena-hard-prompts"]
        assert {f["role"] for f in lm} == {"plan", "review"} and all(f["derived"] for f in lm)
        assert any(f["ids"] == ["claude-opus-5-5"] and f["effort"] == "high" and f["votes"] for f in lm), lm[:2]
        # Equal scores share a place; the next place skips.
        places = sorted((f["score"], f["place"]) for f in lm if f["role"] == "plan")
        for (s1, p1), (s2, p2) in zip(places, places[1:]):
            assert (s1 == s2) == (p1 == p2), places
        # Carry-forward: a build with one source failing keeps the previous facts for it, marked stale.
        out2 = Path(tmp) / "facts2.json"
        saved = SOURCES["aider-polyglot"]["data"]
        try:
            feed2 = build_with_broken("aider-polyglot", out2, out)
        finally:
            SOURCES["aider-polyglot"]["data"] = saved
        stale = next(s for s in feed2["sources"] if s["id"] == "aider-polyglot")
        assert stale["status"] == "stale" and stale["observed"] == feed["sources"][2]["observed"], stale
        assert len([f for f in feed2["facts"] if f["source"] == "aider-polyglot"]) == len(aider)
        # Signing round trip with a throwaway key.
        from cryptography.hazmat.primitives import serialization  # type: ignore

        ed25519 = _keys()
        key = ed25519.Ed25519PrivateKey.generate()
        seed = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw, serialization.NoEncryption())
        public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex()
        sig = sign(out, "test", base64.b64encode(seed).decode("ascii"))
        assert verify(out, sig, public) == "test"
        tampered = Path(tmp) / "tampered.json"
        tampered.write_bytes(out.read_bytes() + b" ")
        try:
            verify(tampered, sig, public)
            raise AssertionError("a tampered feed verified")
        except Exception as e:  # noqa: BLE001
            assert "AssertionError" not in type(e).__name__, e
        assert canonical("openrouter/anthropic/claude-opus-4.5") == "claude-opus-4-5"
        assert canonical("claude-opus-4-5-20251101") == "claude-opus-4-5"
        assert canonical("qwen/qwen3-coder-480b-a35b-instruct:free") == "qwen3-coder-480b-a35b-instruct"
        assert canonical("gemini-2.5-flash-preview-05-20") == "gemini-2-5-flash-preview-05-20"
        assert propose_ids("Claude 4.5 Opus") == ["claude-opus-4-5"] and propose_ids("Claude 3.5 Sonnet") == ["claude-3-5-sonnet"]
        assert propose_ids("Gemini 3.1 Pro Preview") == ["gemini-3-1-pro-preview"]
        assert propose_ids("GPT-5.5 (High)") == ["gpt-5-5"] and effort_of("GPT-5.5 (High)") == "high"
    print("selftest ok")
    return 0


def build_with_broken(source_id: str, out: Path, previous: Path) -> dict:
    """A build in which one fixture is unreadable, so its facts come from `previous`."""
    import tempfile
    import shutil

    with tempfile.TemporaryDirectory() as tmp:
        fixtures = Path(tmp)
        for p in FIXTURES.iterdir():
            if not p.name.startswith(source_id):
                shutil.copy(p, fixtures / p.name)
        (fixtures / fixture_name(source_id)).write_bytes(b"not yaml, not json: {")
        return build(out, previous, None, fixtures=fixtures)


# ---------------------------------------------------------------------------------------------------- main
def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    b = sub.add_parser("build")
    b.add_argument("--out", type=Path, default=FEED_DIR / "v1" / "facts.json")
    b.add_argument("--previous", type=Path, default=None, help="the last published feed, for carrying a failed source forward")
    b.add_argument("--only", default=None, help="one source id")
    s = sub.add_parser("sign")
    s.add_argument("path", type=Path)
    s.add_argument("--key-id", required=True)
    s.add_argument("--seed-env", default="MODEL_FACTS_SIGNING_KEY")
    v = sub.add_parser("verify")
    v.add_argument("path", type=Path)
    v.add_argument("sig", type=Path, nargs="?")
    v.add_argument("--public", required=True, help="the public key, hex")
    k = sub.add_parser("keygen")
    k.add_argument("--seed-file", type=Path, default=None)
    sub.add_parser("names")
    sub.add_parser("selftest")
    args = parser.parse_args(argv)
    if args.command == "build":
        build(args.out, args.previous, args.only)
        return 0
    if args.command == "sign":
        seed = os.environ.get(args.seed_env)
        if not seed:
            raise SystemExit(f"{args.seed_env} is not set")
        print(sign(args.path, args.key_id, seed))
        return 0
    if args.command == "verify":
        print("verified, key", verify(args.path, args.sig or args.path.with_name(args.path.name + ".sig"), args.public))
        return 0
    if args.command == "keygen":
        keygen(args.seed_file)
        return 0
    if args.command == "names":
        return names_report()
    if args.command == "selftest":
        return selftest()
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
