"""Blind review sheets for the paired study (benchmark/study/README.md, step 5).

The folder holds each pair's two diffs as `pair<N>-U.diff` and `pair<N>-P.diff`. This copies them to `review/` as
`pair<N>-X.diff` and `pair<N>-Y.diff`, X and Y drawn at random per pair, writes `review/review-sheet.csv` for the
reviewer, and `key.json` (which of X and Y was Pygg) outside `review/`. Give the reviewer only `review/`.

Review sheet columns: verdict is `merge`, `merge_after_fix` (up to 10 changed lines) or `reject`; defects is how many
the reviewer found.

usage: python -I scripts/study-blind.py FOLDER
"""
import csv
import json
import re
import secrets
import shutil
import sys
from pathlib import Path


def main():
    folder = Path(sys.argv[1])
    pairs = sorted({int(m.group(1)) for p in folder.glob("pair*-*.diff") if (m := re.match(r"pair(\d+)-[UP]\.diff$", p.name))})
    review = folder / "review"
    review.mkdir(exist_ok=True)
    key, rows = {}, []
    for n in pairs:
        u, p = folder / f"pair{n}-U.diff", folder / f"pair{n}-P.diff"
        if not (u.exists() and p.exists()):
            print(f"pair {n}: both pair{n}-U.diff and pair{n}-P.diff are needed; left out")
            continue
        x_is_pygg = secrets.choice([True, False])
        shutil.copyfile(p if x_is_pygg else u, review / f"pair{n}-X.diff")
        shutil.copyfile(u if x_is_pygg else p, review / f"pair{n}-Y.diff")
        key[str(n)] = {"X": "P" if x_is_pygg else "U", "Y": "U" if x_is_pygg else "P"}
        rows += [{"pair": n, "diff": d, "verdict": "", "defects": "", "notes": ""} for d in ("X", "Y")]
    with open(review / "review-sheet.csv", "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=["pair", "diff", "verdict", "defects", "notes"])
        writer.writeheader()
        writer.writerows(rows)
    (folder / "key.json").write_text(json.dumps(key, indent=2), encoding="utf-8")
    print(f"{len(key)} pairs in {review}; the key is in {folder / 'key.json'} (do not give it to the reviewer)")


if __name__ == "__main__":
    main()
