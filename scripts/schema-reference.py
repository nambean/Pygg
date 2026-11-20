"""Writes docs/SCHEMA.md from the migrations, so the schema reference is never edited by hand (review RM-4).

    python scripts/schema-reference.py          # rewrite docs/SCHEMA.md
    python scripts/schema-reference.py --check  # exit 1 if docs/SCHEMA.md is out of date

The migrations in apps/desktop/src-tauri/migrations are applied in order to an empty in-memory SQLite database, as
db.rs does, and every table is listed with the migration that added it and its columns with their key markers.
"""
import pathlib, sqlite3, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MIGRATIONS = ROOT / "apps" / "desktop" / "src-tauri" / "migrations"
OUT = ROOT / "docs" / "SCHEMA.md"


def build():
    conn = sqlite3.connect(":memory:")
    added = {}  # (table, column) -> migration
    for path in sorted(MIGRATIONS.glob("*.sql")):
        before = {(t, c[1]) for t in tables(conn) for c in conn.execute(f'PRAGMA table_info("{t}")')}
        conn.executescript(path.read_text(encoding="utf-8"))
        for t in tables(conn):
            for c in conn.execute(f'PRAGMA table_info("{t}")'):
                if (t, c[1]) not in before:
                    added.setdefault((t, c[1]), path.stem)
    version = len(list(MIGRATIONS.glob("*.sql")))
    lines = [
        "# Store schema",
        "",
        f"> **Status:** Current · generated from the migrations by `scripts/schema-reference.py`; do not edit by hand · schema v{version}",
        "",
        "The local SQLite store (`db.rs`): every table after all migrations, in order. Credentials are never here: a",
        "connection holds only a reference to its key in the OS credential store (ADR-002). Client tokens are stored as",
        "SHA-256 hashes.",
        "",
    ]
    for t in tables(conn):
        cols = list(conn.execute(f'PRAGMA table_info("{t}")'))
        fks = {f[3]: f[2] for f in conn.execute(f'PRAGMA foreign_key_list("{t}")')}
        parts = []
        for cid, name, ctype, notnull, default, pk in cols:
            marks = (["PK"] if pk else []) + ([f"FK `{fks[name]}`"] if name in fks else [])
            parts.append(f"`{name}`" + (f" ({', '.join(marks)})" if marks else ""))
        first = min((added.get((t, c[1]), "") for c in cols), default="")
        lines += [f"## `{t}`", "", f"Added in {first}. Columns: " + ", ".join(parts) + ".", ""]
    return "\n".join(lines)


def tables(conn):
    return [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]


text = build()
if "--check" in sys.argv:
    current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
    if current.replace("\r\n", "\n") != text:
        print("docs/SCHEMA.md is out of date: run python scripts/schema-reference.py")
        sys.exit(1)
    print("docs/SCHEMA.md is up to date")
else:
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {OUT.relative_to(ROOT)}")
