"""A one-shot coding agent for `bench --agent`, for providers that take no tools: web-session connections (ADR-035)
and, for comparison, any API model through the same gateway.

WARNING: its run of 2026-11-13 got the owner's DeepSeek account suspended for three days (24 long prompts in 15
minutes). Pygg's web guard now refuses prompts over 12,000 characters and caps an account's hour by weight, so most
cases are refused on a web session; never point this at an account you need.

    bench --agent "python -I scripts/bench-web-agent.py {prompt_file} {model} {workdir}" ...

It sends one request through the running Pygg gateway: the task, the rules, and every file of the fixture, and asks
for each changed or new file back in full. The files in the reply are written into the worktree; the bench's own
checks then judge them, and a protected file that came back different fails the attempt as it would for any agent.
Nothing runs the code here and nothing is retried: one request per attempt, at the pace Pygg keeps for a web session.

    PYGG_BENCH_KEY_FILE   path of a file holding a Pygg API key (never printed)
    PYGG_BENCH_BASE       the gateway (default http://127.0.0.1:8741)

Exit 0 when the reply held at least one file; 1 when the request failed or the reply held none. The last line of
stdout is a JSON object with the request's timing and the files written, for the raw results.
"""
import json, os, re, sys, time, urllib.error, urllib.request

SKIP_DIRS = {'.git', 'node_modules'}
MAX_FILE = 60_000

prompt_file, model, workdir = sys.argv[1], sys.argv[2], sys.argv[3]
base = os.environ.get('PYGG_BENCH_BASE', 'http://127.0.0.1:8741')
key = open(os.environ['PYGG_BENCH_KEY_FILE'], encoding='utf-8').read().strip()
task = open(prompt_file, encoding='utf-8').read()


def files():
    out = []
    for root, dirs, names in os.walk(workdir):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for n in sorted(names):
            p = os.path.join(root, n)
            rel = os.path.relpath(p, workdir).replace('\\', '/')
            try:
                text = open(p, encoding='utf-8').read()
            except (UnicodeDecodeError, OSError):
                continue
            out.append((rel, text[:MAX_FILE]))
    return out


listing = '\n\n'.join(f"### {rel}\n```\n{text}\n```" for rel, text in files())
message = f"""You are working in a small Node.js repository. You cannot run anything; reply with code only.

## Task

{task}

## Rules

- Reply with every file you change or add, each in full, in this exact form, and nothing else before the first one:

### path/of/file.js
```
<the complete file>
```

- Do not include files you leave unchanged. Never include a file the task says not to change.
- No explanation outside the blocks. The reply is applied by a program.

## The repository

{listing}
"""

body = json.dumps({'model': model, 'stream': True, 'messages': [{'role': 'user', 'content': message}]}).encode()
req = urllib.request.Request(f"{base}/v1/chat/completions", data=body, method='POST',
                             headers={'Authorization': f"Bearer {key}", 'Content-Type': 'application/json'})
started = time.monotonic()
first, text, status, error = None, '', None, None
try:
    with urllib.request.urlopen(req, timeout=900) as r:
        status = r.status
        for raw in r:
            line = raw.decode('utf-8', 'replace').strip()
            if not line.startswith('data:') or line == 'data: [DONE]':
                continue
            try:
                delta = json.loads(line[5:])['choices'][0]['delta'].get('content') or ''
            except (ValueError, KeyError, IndexError):
                continue
            if delta and first is None:
                first = time.monotonic() - started
            text += delta
except urllib.error.HTTPError as e:
    status = e.code
    try:
        err = json.loads(e.read())['error']
        error = err.get('code') or err.get('type') or err.get('message', '')[:120]
    except Exception:
        error = f"HTTP {e.code}"
except Exception as e:
    error = type(e).__name__
total = time.monotonic() - started
# For diagnosis only: the raw reply, kept outside the results when PYGG_BENCH_REPLY_DIR is set.
if os.environ.get('PYGG_BENCH_REPLY_DIR') and text:
    os.makedirs(os.environ['PYGG_BENCH_REPLY_DIR'], exist_ok=True)
    with open(os.path.join(os.environ['PYGG_BENCH_REPLY_DIR'], f"{model.replace('/', '_')}-{int(time.time())}.md"), 'w', encoding='utf-8') as f:
        f.write(text)

# Files come back as a header naming the file, then its content. An API model writes Markdown: "### path" and a
# fenced block. A web session returns what the page shows: the file name on a line of its own, a language word,
# the page's own "Copy" and "Download" buttons, then the code with no fence at all; the next file name ends it.
HEADER = re.compile(r"^\s*(?:#{1,4}\s*)?[`*]*([\w@-][\w./@-]*\.[A-Za-z0-9]{1,8})[`*:]*\s*$")
CHROME = {'copy', 'download', 'text', 'plaintext', 'javascript', 'js', 'json', 'typescript', 'ts', 'md', 'markdown', 'code'}


def parse(reply):
    out, lines, i = [], reply.splitlines(), 0
    while i < len(lines):
        m = HEADER.match(lines[i])
        if not m:
            i += 1
            continue
        rel, i, body, fenced = m.group(1), i + 1, [], False
        # Skip the block's opening: a fence, a language word, the page's buttons, blank lines.
        while i < len(lines) and (not lines[i].strip() or lines[i].strip().lower() in CHROME or lines[i].strip().startswith('```')):
            fenced = fenced or lines[i].strip().startswith('```')
            i += 1
        while i < len(lines):
            line = lines[i]
            if fenced and line.strip().startswith('```'):
                i += 1
                break
            if not fenced and HEADER.match(line) and body:
                break
            body.append(line)
            i += 1
        while body and not body[-1].strip():
            body.pop()
        if body:
            out.append((rel, '\n'.join(body)))
    return out


written = []
for rel, content in parse(text):
    if rel.startswith(('/', '..')) or '..' in rel.split('/'):
        continue
    p = os.path.join(workdir, rel.replace('/', os.sep))
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, 'w', encoding='utf-8', newline='\n') as f:
        f.write(content + '\n')
    written.append(rel)

record = json.dumps({'at': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'model': model, 'status': status, 'error': error,
                     'first_s': round(first, 2) if first is not None else None, 'total_s': round(total, 2), 'reply_chars': len(text),
                     'files': written})
print(record)
if os.environ.get('PYGG_BENCH_REPLY_DIR'):
    with open(os.path.join(os.environ['PYGG_BENCH_REPLY_DIR'], 'requests.jsonl'), 'a', encoding='utf-8') as f:
        f.write(record + '
')
if error:
    print(f"request failed: {error}", file=sys.stderr)
sys.exit(0 if written and not error else 1)
