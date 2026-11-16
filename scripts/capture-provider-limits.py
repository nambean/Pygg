"""Real provider answers for limits, refusals and stalls, saved as fixtures for the gateway's error policy (ROADMAP C1,
P0-2 "fixtures per real provider"), next to the ones scripts/capture-provider-errors.ps1 makes.

Each case is a request that cannot be billed: an account with no balance left (402), a model or file the key may not
use (403), a free model rate-limited upstream (429), and a free model's request given up on by the client after it
was sent (timeout). A case whose answer is not the one wanted (the provider answered 200, or another error) is reported
and not saved.

Not here: Gemini's quota answer (429 RESOURCE_EXHAUSTED). It needs a model the key has no quota for, and on 2026-11-16
the key answered even a Pro preview model, which can bill; the policy reads that answer's RetryInfo in a unit test.

Keys come from Pygg's own store (the connection's reference in store.sqlite3, the secret from the Windows Credential
Manager), or from an environment variable. They are never printed or written: an answer that contains one is not saved,
and fields naming the account (OpenRouter's user_id) are removed.

    python -I scripts/capture-provider-limits.py [--store PATH] [--only CASE,...]

Output: apps/desktop/src-tauri/tests/fixtures/provider-errors/<case>.json
"""

import argparse
import ctypes
import ctypes.wintypes as wt
import datetime
import json
import os
import socket
import sqlite3
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "apps/desktop/src-tauri/tests/fixtures/provider-errors"
KEEP_HEADERS = ("retry-after", "retry-after-ms", "content-type", "x-should-retry", "x-ratelimit-limit-requests",
                "x-ratelimit-remaining-requests", "x-ratelimit-reset-requests")
ACCOUNT_FIELDS = ("user_id", "organization_id", "org_id", "account_id")


class CREDENTIAL(ctypes.Structure):
    _fields_ = [("Flags", wt.DWORD), ("Type", wt.DWORD), ("TargetName", wt.LPWSTR), ("Comment", wt.LPWSTR),
                ("LastWritten", wt.FILETIME), ("CredentialBlobSize", wt.DWORD),
                ("CredentialBlob", ctypes.POINTER(ctypes.c_ubyte)), ("Persist", wt.DWORD),
                ("AttributeCount", wt.DWORD), ("Attributes", ctypes.c_void_p), ("TargetAlias", wt.LPWSTR),
                ("UserName", wt.LPWSTR)]


def vault_secret(reference):
    """The secret the app stored under `reference` (keyring's target `<reference>.SmartTaskRouter`)."""
    if os.name != "nt":
        return None
    advapi = ctypes.WinDLL("advapi32", use_last_error=True)
    pcred = ctypes.POINTER(CREDENTIAL)()
    if not advapi.CredReadW(f"{reference}.SmartTaskRouter", 1, 0, ctypes.byref(pcred)):
        return None
    try:
        blob = bytes(pcred.contents.CredentialBlob[:pcred.contents.CredentialBlobSize])
    finally:
        advapi.CredFree(pcred)
    utf16 = len(blob) % 2 == 0 and blob[1::2].count(0) == len(blob) // 2
    return blob.decode("utf-16-le" if utf16 else "utf-8").strip() or None


def store_keys(store):
    """Connection id → its secret, for the connections that hold one."""
    keys = {}
    if not store.exists():
        return keys
    con = sqlite3.connect(f"file:{store}?mode=ro", uri=True)
    try:
        for pid, ref in con.execute("SELECT id, credential_ref FROM providers WHERE credential_ref IS NOT NULL"):
            secret = vault_secret(ref)
            if secret:
                keys[pid] = secret
    finally:
        con.close()
    return keys


def scrub(value):
    if isinstance(value, dict):
        return {k: scrub(v) for k, v in value.items() if k not in ACCOUNT_FIELDS}
    if isinstance(value, list):
        return [scrub(v) for v in value]
    return value


def post_then_give_up(url, headers, body, wait):
    """Connects and sends the whole request, then waits only `wait` seconds for the answer: a timeout after the request
    left, which is the case the gateway cannot tell from a provider still running it."""
    import http.client
    from urllib.parse import urlsplit
    u = urlsplit(url)
    conn = http.client.HTTPSConnection(u.hostname, u.port or 443, timeout=30)
    started = None
    try:
        conn.connect()
        conn.request("POST", u.path, json.dumps(body).encode(), {"content-type": "application/json", **headers})
        started = time.monotonic()
        conn.sock.settimeout(wait)
        r = conn.getresponse()
        return r.status, dict(r.getheaders()), b"", None, time.monotonic() - started
    except (TimeoutError, socket.timeout) as e:
        return None, {}, b"", f"timeout: {e}" if started else f"transport: {e}", time.monotonic() - (started or 0)
    finally:
        conn.close()


def post(url, headers, body, timeout=60):
    data = json.dumps(body).encode()
    req = urllib.request.Request(url, data, {"content-type": "application/json", **headers}, method="POST")
    started = time.monotonic()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, dict(r.headers), r.read(), None, time.monotonic() - started
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read(), None, time.monotonic() - started
    except (TimeoutError, socket.timeout) as e:
        return None, {}, b"", f"timeout: {e}", time.monotonic() - started
    except urllib.error.URLError as e:
        reason = e.reason
        kind = "timeout" if isinstance(reason, (TimeoutError, socket.timeout)) else "transport"
        return None, {}, b"", f"{kind}: {reason}", time.monotonic() - started


def save(case, url, status, headers, raw, secrets, extra=None):
    text = raw.decode("utf-8", "replace")
    if any(s and s in text for s in secrets):
        print(f"{case:<28} not saved: the answer contains a key")
        return False
    try:
        body = scrub(json.loads(text))
    except ValueError:
        body = text
    kept = {k.lower(): v for k, v in headers.items() if k.lower() in KEEP_HEADERS}
    fixture = {"case": case, "captured": datetime.date.today().isoformat(), "url": url, "status": status,
               "headers": kept, **(extra or {}), "body": body}
    (OUT / f"{case}.json").write_text(json.dumps(fixture, indent=4, ensure_ascii=False) + "\n", encoding="utf-8")
    return True


def chat(model, **more):
    return {"model": model, "max_tokens": 1, "messages": [{"role": "user", "content": "x"}], **more}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--store", default=os.path.expandvars(r"%APPDATA%\app.smarttaskrouter.desktop\store.sqlite3"))
    ap.add_argument("--only", default="")
    args = ap.parse_args()
    only = {c for c in args.only.split(",") if c}
    keys = store_keys(Path(args.store))
    key = lambda pid, env: keys.get(pid) or os.environ.get(env)  # noqa: E731
    secrets = [s for s in [*keys.values(), *(os.environ.get(e) for e in ("GEMINI_API_KEY", "OPENROUTER_API_KEY"))] if s]
    OUT.mkdir(parents=True, exist_ok=True)

    gemini = key("google-gemini", "GEMINI_API_KEY")
    openrouter = key("openrouter", "OPENROUTER_API_KEY")
    gem_native = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
    orouter = "https://openrouter.ai/api/v1/chat/completions"
    # (case, connection's key, url, headers, body, the status wanted, client timeout in seconds)
    cases = [
        ("openrouter-403-harness-only", openrouter, orouter, {"authorization": f"Bearer {openrouter}"},
         chat("thinkingmachines/inkling-2-small:free"), 403, 60),
        ("gemini-403-file", gemini, gem_native, {"x-goog-api-key": gemini or ""},
         {"contents": [{"parts": [{"fileData": {"mimeType": "text/plain",
                                                "fileUri": "https://generativelanguage.googleapis.com/v1beta/files/pygg-no-such-file"}},
                                  {"text": "x"}]}],
          "generationConfig": {"maxOutputTokens": 1}}, 403, 60),
        ("requesty-402-quota", keys.get("requesty-ai"), "https://router.requesty.ai/v1/chat/completions",
         {"authorization": f"Bearer {keys.get('requesty-ai')}"}, chat("alibaba/qwen-max"), 402, 60),
        ("siliconflow-402-quota", keys.get("siliconflow"), "https://api.siliconflow.com/v1/chat/completions",
         {"authorization": f"Bearer {keys.get('siliconflow')}"}, chat("MiniMaxAI/MiniMax-M3"), 402, 60),
        ("openrouter-429-free", openrouter, orouter, {"authorization": f"Bearer {openrouter}"},
         chat("google/gemma-4-26b-a4b-it:free"), 429, 60),
        # Sent in full and then given up on: the client stops waiting before the answer's first byte. A free model, so
        # nothing is billed whether or not the provider runs it.
        ("openrouter-timeout", openrouter, orouter, {"authorization": f"Bearer {openrouter}"},
         chat("google/gemma-4-26b-a4b-it:free", stream=True), None, 0.05),
    ]
    saved = 0
    for case, secret, url, headers, body, wanted, timeout in cases:
        if only and case not in only:
            continue
        if not secret:
            print(f"{case:<28} skipped: no key for this connection")
            continue
        send = post_then_give_up if wanted is None else post
        status, hdrs, raw, failure, secs = send(url, headers, body, timeout)
        if wanted is None:
            if status is None and failure and failure.startswith("timeout"):
                ok = save(case, url, None, {}, b"", secrets,
                          {"failure": "timeout", "request_sent": True, "client_timeout_s": timeout, "waited_s": round(secs, 3)})
                saved += ok
                print(f"{case:<28} timeout after {secs:.2f} s{'' if ok else ' (not saved)'}")
            else:
                print(f"{case:<28} not saved: wanted a timeout, got {status or failure}")
            continue
        if status != wanted:
            print(f"{case:<28} not saved: wanted {wanted}, got {status or failure}")
            continue
        ok = save(case, url, status, hdrs, raw, secrets)
        saved += ok
        print(f"{case:<28} {status}{'' if ok else ' (not saved)'}")
    print(f"{saved} fixture(s) written to {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
