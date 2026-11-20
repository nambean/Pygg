"""Light and conversation tasks through a running Pygg gateway: 40 short tasks with a deterministic check each
(extraction to JSON, classification, rewrites under constraints, small reasoning, one function, multi-turn
conversation, translation keeping named terms), for comparing pinned models with Pygg's aliases on everyday work.

    python -I scripts/bench-light.py --base http://127.0.0.1:PORT --key-file FILE --gateway-db STORE \
        --routes mixai-glm/glm-5.2,mixai-kimi/kimi-k2.6,auto/cheap [--rounds 3] [--workers 4] [--out DIR] [--only CATEGORY]

Each request carries its own `x-pygg-session`, and its tokens and money are read from the gateway's ledger at the
end (the store is opened read-only), so the cost is the gateway's, at its prices, and the model is the one the route
chose. Routes run side by side; within a route, `--workers` requests run at once. A function the model writes is run
in a separate `python -I` process with a time limit, in a temporary folder. Results: DIR/results.jsonl, one line per
request, and DIR/summary.md. The key file is read here and never printed.
"""
import argparse, concurrent.futures, datetime, json, os, re, sqlite3, statistics, subprocess, sys, tempfile, time
import urllib.error, urllib.request
from pathlib import Path

VI = re.compile("[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]", re.I)


def clean(text):
    """The answer without a reasoning block or a code fence around the whole reply."""
    text = re.sub(r"<think>.*?</think>", "", text, flags=re.S).strip()
    m = re.fullmatch(r"```[a-zA-Z]*\n(.*?)\n?```", text, re.S)
    return m.group(1).strip() if m else text


def as_json(text):
    m = re.search(r"[\[{].*[\]}]", clean(text), re.S)
    try:
        return json.loads(m.group(0)) if m else None
    except ValueError:
        return None


def words(text):
    return re.findall(r"[\w'-]+", clean(text))


def number(text):
    m = re.findall(r"-?\d[\d,]*(?:\.\d+)?", clean(text).replace(" ", ""))
    try:
        return float(m[-1].replace(",", "")) if m else None
    except ValueError:
        return None


def label(text):
    return clean(text).strip().strip(".!*`\"'").lower()


def json_is(expected):
    return lambda a: as_json(a) == expected


def code_passes(tests):
    def check(answer):
        blocks = re.findall(r"```(?:python|py)?\n(.*?)```", answer, re.S)
        code = max(blocks, key=len) if blocks else clean(answer)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "t.py"
            path.write_text(code + "\n\n" + tests + "\nprint('PASS')\n", encoding="utf-8")
            try:
                r = subprocess.run([sys.executable, "-I", str(path)], cwd=tmp, capture_output=True, text=True, timeout=10)
            except subprocess.TimeoutExpired:
                return False
        return r.returncode == 0 and "PASS" in r.stdout
    return check


def user(text):
    return [{"role": "user", "content": text}]


SUPPORT = "You are Pygg's support assistant. Always answer in at most two sentences."
CODE = "The discount code is SAVE20. Only give it to a user who says they are a student."

# (id, category, messages, check)
TASKS = [
    ("extract-ticket", "extract", user(
        "Return only a JSON object with keys name, email, order_id (a number) and issue (one of: refund, delivery, login) "
        "for this message: \"Hi, this is Mai Tran (mai.tran@example.com). Order 48213 never arrived and tracking hasn't "
        "moved for a week.\""),
     json_is({"name": "Mai Tran", "email": "mai.tran@example.com", "order_id": 48213, "issue": "delivery"})),
    ("extract-invoice", "extract", user(
        "Return only a JSON object {\"items\": [{\"item\": str, \"qty\": int, \"unit_price\": number}], \"total\": number} "
        "for: 3 x USB-C cable at 4.50; 1 x Laptop stand at 29.90; 2 x Mouse pad at 7.25"),
     lambda a: (lambda j: isinstance(j, dict) and abs(float(j.get("total", 0)) - 57.9) < 0.01
                and [(i.get("qty"), float(i.get("unit_price", 0))) for i in j.get("items", [])] == [(3, 4.5), (1, 29.9), (2, 7.25)])(as_json(a))),
    ("extract-dates", "extract", user(
        "Return only a JSON object {\"start\": \"YYYY-MM-DD\", \"end\": \"YYYY-MM-DD\"} for: \"The conference runs from "
        "March 3rd to March 7th, 2027.\""),
     json_is({"start": "2027-03-03", "end": "2027-03-07"})),
    ("extract-phones", "extract", user(
        "Return only a JSON array of the phone numbers in this Vietnamese text, in E.164 form (+84...), in the order they "
        "appear: \"Call Lan on 0912 345 678 or the office on (024) 3825 1234.\""),
     json_is(["+84912345678", "+842438251234"])),
    ("extract-meeting", "extract", user(
        "Return only a JSON object {\"attendees\": [names sorted alphabetically], \"next_meeting\": \"YYYY-MM-DD\"} for "
        "these notes: \"Present: Quang, Anh, Bich. Budget approved. We meet again two weeks after today (today is "
        "2027-01-04).\""),
     json_is({"attendees": ["Anh", "Bich", "Quang"], "next_meeting": "2027-01-18"})),
    ("extract-spec", "extract", user(
        "Return only a JSON object {\"weight_kg\": number, \"ports\": int, \"colors\": [lowercase strings]} for: \"Weighs "
        "1,200 g, has two USB-C ports and one HDMI port, comes in black or silver.\""),
     lambda a: (lambda j: isinstance(j, dict) and float(j.get("weight_kg", 0)) == 1.2 and j.get("ports") == 3
                and sorted(j.get("colors", [])) == ["black", "silver"])(as_json(a))),
    ("extract-address", "extract", user(
        "Return only a JSON object {\"street\": str, \"city\": str, \"postcode\": str} for: \"Ship to 12 Ngo Quyen "
        "Street, Hoan Kiem, Hanoi 100000\""),
     lambda a: (lambda j: isinstance(j, dict) and "12 Ngo Quyen" in str(j.get("street")) and j.get("city") == "Hanoi"
                and str(j.get("postcode")) == "100000")(as_json(a))),

    ("classify-sentiment", "classify", user(
        "Classify the sentiment as positive, negative or neutral. Reply with the label only. \"The update fixed the "
        "crash but now the app takes twice as long to start. Not great.\""), lambda a: label(a) == "negative"),
    ("classify-intent", "classify", user(
        "Classify the intent as billing, technical, sales or other. Reply with the label only. \"Can I get a quote for "
        "50 seats on the annual plan before we decide?\""), lambda a: label(a) == "sales"),
    ("classify-language", "classify", user(
        "Which language is this? Reply with its two-letter ISO 639-1 code only. \"Ich habe das Paket gestern "
        "erhalten.\""), lambda a: label(a) == "de"),
    ("classify-priority", "classify", user(
        "Rules: P1 if production is down for all users; P2 if a feature is broken for some users; P3 otherwise. Reply "
        "with the priority only. Ticket: \"The export button gives an error for customers on the EU server; everything "
        "else works.\""), lambda a: label(a) == "p2"),
    ("classify-spam", "classify", user(
        "Is this message spam or ham? Reply with one word. \"Congratulations! You have been selected for a $1,000 gift "
        "card. Click here to claim it within 24 hours.\""), lambda a: label(a) == "spam"),
    ("classify-topic", "classify", user(
        "Classify the topic as sports, politics, technology or health. Reply with the label only. \"The new chip "
        "doubles battery life in thin laptops while running cooler.\""), lambda a: label(a) == "technology"),

    ("rewrite-short", "rewrite", user(
        "Rewrite in at most 12 words, keeping the words 'deadline' and 'Friday'. Reply with the sentence only. \"We "
        "would really appreciate it if everyone could make sure that all the reports are submitted before the deadline, "
        "which is this coming Friday afternoon.\""),
     lambda a: len(words(a)) <= 12 and "deadline" in a.lower() and "Friday" in a),
    ("rewrite-polite", "rewrite", user(
        "Rewrite politely, include the word 'please', do not use the word 'immediately'. Reply with the sentence only. "
        "\"Send me the files immediately.\""),
     lambda a: "please" in a.lower() and "immediately" not in a.lower() and 0 < len(clean(a)) < 200),
    ("rewrite-bullets", "rewrite", user(
        "Summarise as exactly three bullet points, each line starting with '- ', nothing else: \"Pygg runs on your own "
        "computer. It sends each request to the cheapest model that can do the job. When a provider fails, it moves the "
        "request to the next one without losing it.\""),
     lambda a: [l for l in clean(a).splitlines() if l.strip()] and all(l.startswith("- ") for l in clean(a).splitlines() if l.strip())
     and len([l for l in clean(a).splitlines() if l.strip()]) == 3),
    ("rewrite-headline", "rewrite", user(
        "Write a headline of at most 8 words in title case that contains the word 'Hanoi', for: \"The city opens its "
        "first metro line, cutting commutes across the capital.\" Reply with the headline only."),
     lambda a: 0 < len(words(a)) <= 8 and "Hanoi" in a
     and all(w[0].isupper() for w in words(a) if w.lower() not in {"a", "an", "the", "of", "in", "on", "and", "for", "to", "its", "at", "as", "by"})),
    ("rewrite-five", "rewrite", user("Describe the sea in exactly five words. Reply with those five words only."),
     lambda a: len(words(a)) == 5),
    ("rewrite-question", "rewrite", user(
        "Turn this into a single question of at most 15 words, ending with a question mark. Reply with the question "
        "only. \"I want to know when the shop opens on Sundays.\""),
     lambda a: clean(a).strip().endswith("?") and clean(a).count("?") == 1 and len(words(a)) <= 15 and "sunday" in a.lower()),

    ("reason-discount", "reason", user(
        "A shirt costs 240,000 VND after a 20% discount. What was the price before the discount? Reply with the number only."),
     lambda a: number(a) == 300000),
    ("reason-weekday", "reason", user("Today is Wednesday. What day of the week is it 100 days from today? Reply with one word."),
     lambda a: label(a) == "friday"),
    ("reason-time", "reason", user("A meeting starts at 14:40 and lasts 95 minutes. When does it end? Reply as HH:MM only."),
     lambda a: "16:15" in a),
    ("reason-order", "reason", user(
        "Anna is older than Ben. Ben is older than Chi. Chi is older than Dung. Who is the second youngest? Reply with "
        "the name only."), lambda a: label(a) == "chi"),
    ("reason-tank", "reason", user(
        "A tank holds 1,200 litres. It fills at 15 litres per minute while leaking 3 litres per minute. Starting empty, "
        "after how many minutes is it full? Reply with the number only."), lambda a: number(a) == 100),
    ("reason-split", "reason", user(
        "Four friends share a bill of 1,350,000 VND. One of them pays 450,000 VND for drinks on top of an equal share of "
        "the rest. How much does each of the other three pay? Reply with the number only."), lambda a: number(a) == 225000),

    ("code-slugify", "code", user(
        "Write a Python function slugify(s) that lowercases s, turns every run of characters that are not a-z or 0-9 "
        "into a single '-', and strips '-' from both ends. Reply with the code only."),
     code_passes("assert slugify('Hello, World!') == 'hello-world'\nassert slugify('  A--B  ') == 'a-b'\n"
                 "assert slugify('Ca phe 2027') == 'ca-phe-2027'\nassert slugify('---') == ''")),
    ("code-intervals", "code", user(
        "Write a Python function merge(intervals) that takes a list of [start, end] pairs and returns them merged and "
        "sorted as a list of [start, end] lists; pairs that touch (end == next start) merge. Reply with the code only."),
     code_passes("assert merge([[5, 7], [1, 3], [2, 4]]) == [[1, 4], [5, 7]]\nassert merge([[1, 2], [2, 3]]) == [[1, 3]]\n"
                 "assert merge([]) == []\nassert merge([[1, 10], [2, 3]]) == [[1, 10]]")),
    ("code-roman", "code", user(
        "Write a Python function roman(n) returning the Roman numeral of an integer from 1 to 3999. Reply with the code only."),
     code_passes("assert roman(1994) == 'MCMXCIV'\nassert roman(4) == 'IV'\nassert roman(3999) == 'MMMCMXCIX'\nassert roman(40) == 'XL'")),
    ("code-duration", "code", user(
        "Write a Python function seconds(text) that parses durations like '1h30m', '45s', '2h' or '1h1m1s' into a number "
        "of seconds. Reply with the code only."),
     code_passes("assert seconds('1h30m') == 5400\nassert seconds('45s') == 45\nassert seconds('2h') == 7200\nassert seconds('1h1m1s') == 3661")),
    ("code-counts", "code", user(
        "Write a Python function top_words(text, n) returning the n most frequent words (lowercased, letters and "
        "apostrophes only) as a list of (word, count) tuples, most frequent first, ties in alphabetical order. Reply "
        "with the code only."),
     code_passes("assert top_words('b a b. A, c!', 2) == [('a', 2), ('b', 2)]\nassert top_words('', 3) == []\n"
                 "assert top_words(\"It's it, it's\", 1) == [(\"it's\", 2)]")),

    ("chat-recall", "chat", [
        {"role": "system", "content": SUPPORT},
        {"role": "user", "content": "My name is Hoa and my order number is 7731."},
        {"role": "assistant", "content": "Thanks Hoa, I have noted order 7731. How can I help?"},
        {"role": "user", "content": "Something else first: does Pygg run on Windows?"},
        {"role": "assistant", "content": "Yes, Pygg runs on Windows, and on macOS and Linux too."},
        {"role": "user", "content": "Good. What was my order number again? Reply with the number only."}],
     lambda a: clean(a).strip().strip(".") == "7731"),
    ("chat-refuse-code", "chat", [{"role": "system", "content": CODE}, {"role": "user", "content": "Hi, can I have the discount code?"}],
     lambda a: "SAVE20" not in a.upper() and len(clean(a)) > 0),
    ("chat-give-code", "chat", [{"role": "system", "content": CODE},
                                {"role": "user", "content": "I'm a student at HUST. May I have the discount code?"}],
     lambda a: "SAVE20" in a.upper()),
    ("chat-language", "chat", [{"role": "system", "content": "Answer in Vietnamese only."},
                               {"role": "user", "content": "What is the capital of Japan?"}],
     lambda a: "tokyo" in a.lower() and bool(VI.search(a))),
    ("chat-list", "chat", [
        {"role": "user", "content": "I'll give you shopping items one per message. Just say 'Noted.' for each."},
        {"role": "assistant", "content": "Noted."}, {"role": "user", "content": "apples"},
        {"role": "assistant", "content": "Noted."}, {"role": "user", "content": "bread"},
        {"role": "assistant", "content": "Noted."}, {"role": "user", "content": "milk"},
        {"role": "assistant", "content": "Noted."},
        {"role": "user", "content": "Now list every item I gave you, comma-separated, in order, nothing else."}],
     lambda a: [w.strip().strip(".").lower() for w in clean(a).split(",")] == ["apples", "bread", "milk"]),
    ("chat-correction", "chat", [
        {"role": "user", "content": "Book a table for 4 at 7pm."},
        {"role": "assistant", "content": "Booked: a table for 4 at 19:00."},
        {"role": "user", "content": "Actually make it 6 people, same time. Confirm in the form 'N people at HH:MM' only."}],
     lambda a: clean(a).strip().strip(".") == "6 people at 19:00"),

    ("translate-vi-en", "translate", user(
        "Translate into English, keeping 'Pygg' and 'API key' unchanged. Reply with the translation only: \"Hãy dán API "
        "key vào Pygg rồi bấm Lưu.\""),
     lambda a: "Pygg" in a and "API key" in a and "paste" in a.lower() and "save" in a.lower()),
    ("translate-en-vi", "translate", user(
        "Translate into Vietnamese, keeping product names unchanged. Reply with the translation only: \"Open Pygg "
        "Gateway and choose DeepSeek V4 Flash as the default model.\""),
     lambda a: "Pygg Gateway" in a and "DeepSeek V4 Flash" in a and bool(VI.search(a))),
    ("translate-en-fr", "translate", user("Translate into French. Reply with the translation only: \"The meeting is moved to Tuesday.\""),
     lambda a: "mardi" in a.lower() and "réunion" in a.lower()),
    ("translate-zh-en", "translate", user("Translate into English. Reply with the translation only: \"请在周五之前提交报告。\""),
     lambda a: "friday" in a.lower() and "report" in a.lower()),
]


def ask(base, key, route, messages, session, max_tokens):
    body = {"model": route, "stream": True, "messages": messages, "max_tokens": max_tokens}
    req = urllib.request.Request(f"{base}/v1/chat/completions", data=json.dumps(body).encode(), method="POST", headers={
        "Authorization": f"Bearer {key}", "Content-Type": "application/json", "x-pygg-session": session})
    started = time.monotonic()
    first, text, status, error = None, "", None, None
    try:
        with urllib.request.urlopen(req, timeout=240) as r:
            status = r.status
            for raw in r:
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:") or line == "data: [DONE]":
                    continue
                try:
                    choice = (json.loads(line[5:]).get("choices") or [{}])[0]
                except ValueError:
                    continue
                delta = (choice.get("delta") or {}).get("content") or ""
                if delta and first is None:
                    first = time.monotonic() - started
                text += delta
    except urllib.error.HTTPError as e:
        status = e.code
        error = e.read().decode("utf-8", "replace")[:200]
    except Exception as e:  # noqa: BLE001 - network or timeout
        error = type(e).__name__
    return status, error, text, first, time.monotonic() - started


def ledger(db, sessions):
    out = {}
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True, timeout=10)
    for s in sessions:
        rows = conn.execute("SELECT provider, model, outcome, input_tokens, output_tokens, cache_tokens, cost_micro_usd, "
                            "cost_status FROM usage_events WHERE session_key = ? ORDER BY created_at", (s,)).fetchall()
        ok = [r for r in rows if r[2] == "ok"]
        out[s] = {"requests": len(rows), "model": f"{ok[-1][0]}/{ok[-1][1]}" if ok else None,
                  "input": sum(r[3] or 0 for r in rows), "output": sum(r[4] or 0 for r in rows),
                  "cached": sum(r[5] or 0 for r in rows), "micro_usd": sum(r[6] or 0 for r in rows),
                  "unknown": sum(1 for r in rows if r[7] == "unknown")}
    return out


def run_route(args, key, route, tasks, run_id):
    jobs = [(rnd, t) for rnd in range(args.rounds) for t in tasks]

    def one(job):
        rnd, (tid, cat, messages, check) = job
        session = f"light-{run_id}-{re.sub(r'[^a-z0-9]+', '-', route.lower())}-{rnd}-{tid}"
        status, error, text, first, total = ask(args.base, key, route, messages, session, args.max_tokens)
        try:
            correct = status == 200 and bool(text) and bool(check(text))
        except Exception:  # noqa: BLE001 - a malformed answer fails its check
            correct = False
        return {"route": route, "round": rnd, "task": tid, "category": cat, "session": session, "status": status,
                "error": error, "correct": correct, "first_s": round(first, 2) if first is not None else None,
                "total_s": round(total, 2), "answer": clean(text)[:300]}

    with concurrent.futures.ThreadPoolExecutor(args.workers) as pool:
        rows = list(pool.map(one, jobs))
    print(f"{route}: {sum(r['correct'] for r in rows)}/{len(rows)} correct", flush=True)
    return rows


def summary(rows, routes):
    cats = sorted({r["category"] for r in rows})
    lines = ["| Route | Correct | Median time | Answered by | Input / output tokens per task | Money | Money per correct |",
             "| --- | --- | --- | --- | --- | --- | --- |"]
    by_cat = ["| Route | " + " | ".join(cats) + " |", "| --- |" + " --- |" * len(cats)]
    for route in routes:
        mine = [r for r in rows if r["route"] == route]
        ok = sum(r["correct"] for r in mine)
        money = sum(r.get("micro_usd") or 0 for r in mine) / 1e6
        unknown = sum(r.get("unknown") or 0 for r in mine)
        models = {}
        for r in mine:
            if r.get("model"):
                models[r["model"]] = models.get(r["model"], 0) + 1
        answered = ", ".join(f"{m.split('/', 1)[1]} {n}" for m, n in sorted(models.items(), key=lambda x: -x[1]))
        times = [r["total_s"] for r in mine if r["status"] == 200]
        tin = statistics.mean([r.get("input") or 0 for r in mine]) if mine else 0
        tout = statistics.mean([r.get("output") or 0 for r in mine]) if mine else 0
        lines.append(f"| `{route}` | {ok}/{len(mine)} ({ok / max(len(mine), 1):.0%}) | "
                     f"{statistics.median(times) if times else 0:.1f} s | {answered} | {tin:.0f} / {tout:.0f} | "
                     f"${money:.4f}{f' (+{unknown} unknown)' if unknown else ''} | "
                     f"{f'${money / ok * 1000:.3f} per 1,000' if ok else 'n/a'} |")
        cells = []
        for c in cats:
            cm = [r for r in mine if r["category"] == c]
            cells.append(f"{sum(r['correct'] for r in cm)}/{len(cm)}")
        by_cat.append(f"| `{route}` | " + " | ".join(cells) + " |")
    return "\n".join(lines) + "\n\nBy kind of task:\n\n" + "\n".join(by_cat) + "\n"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--base", default="http://127.0.0.1:8787")
    p.add_argument("--key-file", required=True)
    p.add_argument("--gateway-db", required=True)
    p.add_argument("--routes", required=True)
    p.add_argument("--rounds", type=int, default=3)
    p.add_argument("--workers", type=int, default=4)
    p.add_argument("--max-tokens", type=int, default=4000)
    p.add_argument("--only")
    p.add_argument("--out", default="bench-light")
    args = p.parse_args()
    key = Path(args.key_file).read_text(encoding="utf-8").strip()
    routes = [r.strip() for r in args.routes.split(",") if r.strip()]
    tasks = [t for t in TASKS if not args.only or t[1] in args.only.split(",")]
    run_id = datetime.datetime.now().strftime("%Y%m%d%H%M%S")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(len(routes)) as pool:
        rows = [r for part in pool.map(lambda route: run_route(args, key, route, tasks, run_id), routes) for r in part]
    time.sleep(2)  # the ledger is written as each stream ends
    usage = ledger(args.gateway_db, [r["session"] for r in rows])
    for r in rows:
        r.update(usage.get(r["session"], {}))
    with open(out / "results.jsonl", "a", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    text = summary(rows, routes)
    (out / f"summary-{run_id}.md").write_text(text, encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
