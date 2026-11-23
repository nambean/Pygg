"""ROADMAP focus item 6: the command-line path from nothing to a first routed request, timed step by step, in a
clean profile (its own home, AppData and npm prefix in a temporary folder), with a local Ollama as the provider.

Steps: install @pygg/cli from npm, add the Ollama connection, start the gateway on a spare port, create a client key,
then ask `auto` a short question until it answers. Every step's time, exit code and the end of its output are
recorded; the first request is retried after each hint Pygg gives, so the steps a newcomer would need show.

usage: python -I scripts/proof-first-request.py OUT.jsonl [--version dev | --exe PATH] [--port 8761]
`--exe` runs a local build of pygg-cli instead of installing from npm (to measure a change before it ships).
Nothing outside the temporary folder is written; no provider key is used (Ollama needs none).
"""
import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    args = argparse.ArgumentParser()
    args.add_argument("out")
    args.add_argument("--version", default="dev")
    args.add_argument("--port", type=int, default=8761)
    args.add_argument("--exe")
    a = args.parse_args()
    root = Path(tempfile.mkdtemp(prefix="pygg-fresh-"))
    env = dict(os.environ)
    for name, sub in (("USERPROFILE", "home"), ("HOME", "home"), ("APPDATA", "roaming"), ("LOCALAPPDATA", "local"),
                      ("npm_config_prefix", "npm"), ("npm_config_cache", "npm-cache")):
        (root / sub).mkdir(exist_ok=True)
        env[name] = str(root / sub)
    env["PATH"] = str(root / "npm") + os.pathsep + str(root / "npm" / "bin") + os.pathsep + env["PATH"]
    npm = shutil.which("npm", path=env["PATH"]) or "npm"
    rows, started = [], time.monotonic()

    def step(name, command, timeout=600, quiet=False):
        t0 = time.monotonic()
        exe = a.exe if (a.exe and command[0] == "pygg") else shutil.which(command[0], path=env["PATH"]) or command[0]
        done = subprocess.run([exe, *command[1:]], env=env, capture_output=True, text=True, encoding="utf-8",
                              errors="replace", timeout=timeout)
        row = {"step": name, "command": " ".join(command), "exit": done.returncode,
               "seconds": round(time.monotonic() - t0, 1), "sinceStart": round(time.monotonic() - started, 1),
               "tail": (done.stdout + done.stderr).strip().splitlines()[-6:]}
        if quiet:
            row["tail"] = ["(output hidden: it holds a key)"]
        rows.append(row)
        print(f"[{row['sinceStart']:>6}s] {name}: exit {row['exit']} in {row['seconds']}s")
        for line in row["tail"]:
            print(f"           | {line}")
        return done

    if not a.exe:
        step("install", [npm, "install", "-g", f"@pygg/cli@{a.version}"], timeout=900)
    step("version", ["pygg", "--version"])
    step("add Ollama", ["pygg", "gateway", "add-connection", "--preset", "ollama"])
    step("list", ["pygg", "gateway", "list"])
    exe = a.exe or shutil.which("pygg", path=env["PATH"]) or "pygg"
    server = subprocess.Popen([exe, "gateway", "serve", "--port", str(a.port)], env=env, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL)
    try:
        time.sleep(3)
        made = step("create key", ["pygg", "gateway", "create-client", "--name", "first-try"], quiet=True)
        key = next((w for line in made.stdout.splitlines() for w in line.split() if w.startswith(("str-", "sk-"))), "")

        def ask(name):
            t0 = time.monotonic()
            body = json.dumps({"model": "auto", "max_tokens": 32,
                               "messages": [{"role": "user", "content": "Say ok."}]}).encode()
            request = urllib.request.Request(f"http://127.0.0.1:{a.port}/v1/chat/completions", data=body,
                                             headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
            try:
                with urllib.request.urlopen(request, timeout=300) as r:
                    status, text, route = r.status, r.read().decode()[:300], r.headers.get("x-pygg-route")
            except urllib.error.HTTPError as e:
                status, text, route = e.code, e.read().decode()[:400], None
            except OSError as e:
                status, text, route = 0, str(e)[:300], None
            row = {"step": name, "status": status, "route": route, "seconds": round(time.monotonic() - t0, 1),
                   "sinceStart": round(time.monotonic() - started, 1), "tail": [text]}
            rows.append(row)
            print(f"[{row['sinceStart']:>6}s] {name}: HTTP {status} via {route}\n           | {text[:200]}")
            return status

        # A newcomer does what Pygg tells them: a `pygg ...` command printed when the key was made, then each one an
        # error names, in order, never one twice; with none left to follow, the commands the guide lists.
        done = set()

        def follow(text, label):
            for found in re.findall(r"(pygg gateway [^`\n]+)", text):
                command = shlex.split(found.strip().rstrip("."))
                if "CONNECTION" in found or tuple(command) in done:
                    continue
                done.add(tuple(command))
                step(f"{label}: {found.strip()}", command)
                return True
            return False

        follow(made.stdout, "follow the key's hint")
        status = ask("first request")
        fallback = iter([["pygg", "gateway", "refresh-models", "--id", "ollama"], ["pygg", "gateway", "lookup"]])
        for turn in range(4):
            if status == 200:
                break
            text = rows[-1]["tail"][0]
            if not follow(text, "follow the error's hint"):
                command = next(fallback, None)
                if command is None:
                    break
                done.add(tuple(command))
                step("guide: " + " ".join(command), command)
            status = ask(f"request {turn + 2}")
    finally:
        # npm's launcher runs the native program as its child: stop the whole tree, or the child keeps the port.
        if os.name == "nt":
            subprocess.run(["taskkill", "/F", "/T", "/PID", str(server.pid)], capture_output=True)
        else:
            server.terminate()
        server.wait(timeout=20)
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.writelines(json.dumps(r) + "\n" for r in rows)
    shutil.rmtree(root, ignore_errors=True)
    print(f"total {round(time.monotonic() - started, 1)} s; rows in {a.out}")


if __name__ == "__main__":
    main()
