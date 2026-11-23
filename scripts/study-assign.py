"""Which task of each pair a study participant does with their usual setup (U) and which through Pygg (P).

The draw uses the operating system's randomness, so the person cannot steer it; it is printed and saved, so it can be
checked afterwards (benchmark/study/README.md, step 2).

usage: python -I scripts/study-assign.py --person P01 --pairs 3 [--out assignment-P01.json]
"""
import argparse
import json
import sys
import secrets
from datetime import datetime, timezone


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    args = argparse.ArgumentParser()
    args.add_argument("--person", required=True)
    args.add_argument("--pairs", type=int, default=3)
    args.add_argument("--out")
    a = args.parse_args()
    pairs = []
    for n in range(1, a.pairs + 1):
        first = secrets.choice("UP")
        pairs.append({"pair": n, "a": first, "b": "P" if first == "U" else "U"})
        print(f"pair {n}: task a -> {first}, task b -> {pairs[-1]['b']}")
    record = {"person": a.person, "drawnAt": datetime.now(timezone.utc).isoformat(timespec="seconds"), "pairs": pairs}
    out = a.out or f"assignment-{a.person}.json"
    with open(out, "w", encoding="utf-8") as f:
        json.dump(record, f, indent=2)
    print(f"saved {out}")


if __name__ == "__main__":
    main()
