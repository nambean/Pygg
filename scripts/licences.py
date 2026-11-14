"""Third-party licence inventory for what Pygg ships.

Lists every Rust crate built into the Windows binaries (`cargo metadata`, normal dependencies only, resolved for
x86_64-pc-windows-msvc) and every npm package bundled into the interface (the desktop app's `dependencies`, walked
through node_modules), with each package's licence and the text of its licence files, de-duplicated.

    python scripts/licences.py           # write apps/desktop/src-tauri/licenses/THIRD-PARTY-LICENSES.md
    python scripts/licences.py --check   # fail if the file is out of date or a licence is not on the allow list

The installer ships the file (tauri.conf.json bundle.resources), and Settings → About points to it.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.join(ROOT, "apps", "desktop", "src-tauri")
UI = os.path.join(ROOT, "apps", "desktop")
OUT = os.path.join(CRATE, "licenses", "THIRD-PARTY-LICENSES.md")
TARGET = "x86_64-pc-windows-msvc"

# Permissive licences Pygg (MIT) can ship in a binary with their notices. Anything else stops the check for a decision.
ALLOWED = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib",
    "Unicode-3.0", "Unicode-DFS-2016", "CC0-1.0", "Unlicense", "BSL-1.0", "0BSD", "MPL-2.0", "CDLA-Permissive-2.0",
    "OFL-1.1", "bzip2-1.0.6",
}
LICENCE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-._].*)?$", re.I)


def licence_terms(expr):
    """The licences an SPDX expression names (MIT OR Apache-2.0 → both); an OR needs one allowed, an AND all."""
    expr = (expr or "").replace("/", " OR ").strip()
    if not expr:
        return []
    return [t.strip("() ") for t in re.split(r"\s+(?:AND|OR)\s+", expr) if t.strip("() ")]


def allowed(expr):
    expr = (expr or "").replace("/", " OR ")
    if not expr.strip():
        return False
    # Evaluate: every AND part needs at least one allowed alternative among its ORs.
    for conj in re.split(r"\s+AND\s+", expr.replace("(", "").replace(")", "")):
        options = [o.strip() for o in re.split(r"\s+OR\s+", conj)]
        if not any(o in ALLOWED for o in options):
            return False
    return True


def licence_texts(directory):
    texts = []
    try:
        names = sorted(os.listdir(directory))
    except OSError:
        return texts
    for name in names:
        path = os.path.join(directory, name)
        if LICENCE_FILE.match(name) and os.path.isfile(path):
            with open(path, encoding="utf-8", errors="replace") as f:
                texts.append((name, f.read().replace("\r\n", "\n").strip()))
    return texts


def rust_packages():
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
        cwd=CRATE, capture_output=True, text=True, check=True, encoding="utf-8").stdout)
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    roots = set(meta["workspace_members"])
    seen, stack = set(), list(roots)
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            # Normal dependencies only: build scripts and dev dependencies are not shipped.
            if any(k["kind"] in (None, "normal") for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    out = []
    for pid in seen - roots:
        p = packages[pid]
        out.append({
            "ecosystem": "Rust", "name": p["name"], "version": p["version"],
            "licence": p.get("license") or ("see " + p["license_file"] if p.get("license_file") else ""),
            "url": p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}",
            "texts": licence_texts(os.path.dirname(p["manifest_path"])),
        })
    return out


def npm_packages():
    with open(os.path.join(UI, "package.json"), encoding="utf-8") as f:
        top = json.load(f).get("dependencies", {})
    out, seen = [], set()

    def find(name, base):
        # Node's resolution: node_modules next to the requiring package, then up the tree (pnpm links them so).
        d = base
        while True:
            candidate = os.path.join(d, "node_modules", name)
            if os.path.isfile(os.path.join(candidate, "package.json")):
                return os.path.realpath(candidate)
            parent = os.path.dirname(d)
            if parent == d:
                return None
            d = parent

    def visit(name, base):
        path = find(name, base)
        if not path or path in seen:
            return
        seen.add(path)
        with open(os.path.join(path, "package.json"), encoding="utf-8") as f:
            pkg = json.load(f)
        lic = pkg.get("license")
        if isinstance(lic, dict):
            lic = lic.get("type")
        if not lic and isinstance(pkg.get("licenses"), list):
            lic = " OR ".join(x.get("type", "") for x in pkg["licenses"])
        repo = pkg.get("repository")
        if isinstance(repo, dict):
            repo = repo.get("url")
        out.append({
            "ecosystem": "npm", "name": pkg["name"], "version": pkg["version"], "licence": lic or "",
            "url": (repo or pkg.get("homepage") or f"https://www.npmjs.com/package/{pkg['name']}").replace("git+", ""),
            "texts": licence_texts(path),
        })
        for dep in pkg.get("dependencies", {}):
            visit(dep, path)

    for name in top:
        visit(name, UI)
    return out


def render(packages):
    packages = sorted(packages, key=lambda p: (p["ecosystem"], p["name"].lower(), p["version"]))
    lines = [
        "# Third-party licences",
        "",
        "Pygg is MIT-licensed. The Windows app and its command line include the open-source packages below, each under",
        "its own licence; their licence texts follow the list. Generated by `scripts/licences.py` from `Cargo.lock` and",
        "the interface's npm dependencies; do not edit by hand. Code that Pygg ports or follows is credited in",
        "THIRD_PARTY_NOTICES.md in the source repository.",
        "",
        f"{sum(p['ecosystem'] == 'Rust' for p in packages)} Rust crates and {sum(p['ecosystem'] == 'npm' for p in packages)} npm packages.",
        "",
        "| Package | Version | Licence | Source | Text |",
        "|---|---|---|---|---|",
    ]
    texts, order = {}, []
    for p in packages:
        refs = []
        for name, body in p["texts"]:
            key = hashlib.sha256(body.encode("utf-8")).hexdigest()[:12]
            if key not in texts:
                texts[key] = (body, [])
                order.append(key)
            texts[key][1].append(f"{p['name']} {p['version']}")
            refs.append(f"[{key}](#t-{key})")
        refs = refs or ["(licence named only)"]
        lic = (p["licence"] or "unknown").replace("|", "/")
        lines.append(f"| {p['ecosystem']}: {p['name']} | {p['version']} | {lic} | {p['url']} | {', '.join(refs)} |")
    lines += ["", "## Licence texts", ""]
    for key in order:
        body, users = texts[key]
        shown = ", ".join(users[:12]) + (f" and {len(users) - 12} more" if len(users) > 12 else "")
        lines += [f'<a id="t-{key}"></a>', f"### {key}", "", f"Used by {shown}.", "", "```text", body.replace("```", "'''"), "```", ""]
    return "\n".join(lines).rstrip() + "\n"


def main():
    check = "--check" in sys.argv[1:]
    packages = rust_packages() + npm_packages()
    refused = [p for p in packages if not allowed(p["licence"])]
    text = render(packages)
    if check:
        current = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else ""
        problems = []
        if current != text:
            problems.append(f"{os.path.relpath(OUT, ROOT)} is out of date: run python scripts/licences.py")
        problems += [f"not on the allow list: {p['ecosystem']} {p['name']} {p['version']} ({p['licence'] or 'no licence'})"
                     for p in refused]
        for line in problems:
            print(line, file=sys.stderr)
        sys.exit(1 if problems else 0)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"{len(packages)} packages, {len(refused)} to decide; wrote {os.path.relpath(OUT, ROOT)}")
    for p in refused:
        print(f"  decide: {p['ecosystem']} {p['name']} {p['version']} ({p['licence'] or 'no licence'})")


if __name__ == "__main__":
    main()
