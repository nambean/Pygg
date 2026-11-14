"""A fault-injecting fake LLM provider for the resilience benchmark.

Speaks OpenAI's Chat Completions (`/<provider>/v1/chat/completions`, whole and streamed) and lists models
(`/<provider>/v1/models`). Several providers share one process: the first path segment names the provider
(`a`, `b`, `c`, ...), and the bearer token names the key. What each provider or key does is set at run time:

    POST /_admin/config   {"latency_ms": 40, "rules": {"a": {"mode": "429", "retry_after": 60},
                                                        "a:key-a2": {"mode": "ok"}}}
    POST /_admin/reset    forget the request log and the request counters
    GET  /_admin/log      every request seen, one JSON object per line

A rule for `provider:key` wins over the provider's rule. Modes:

    ok                 answer normally
    429 | 402 | 401 | 403 | 500 | 502 | 503 | 504
                       answer that status with an OpenAI-style error body (429 carries Retry-After when set)
    quota              429 with OpenAI's `insufficient_quota` code (an account out of credit)
    flaky              500 for a share `p` of requests (deterministic: every request whose counter hits the
                       share), otherwise ok
    hang               wait `seconds` before sending anything, then answer ok
    drop               read the request, then close the connection without an answer
    midstream          for a streamed request: status 200 and two content chunks, then close the connection
                       (a whole request is answered as `drop`)

It also speaks Anthropic's Messages API (`/<provider>/v1/messages`, whole and streamed) with a simulated prompt
cache, for the token benchmark: the prompt is cut into blocks (tools, system, then each message's content blocks); a
`cache_control` on a block, or a top-level `cache_control` (automatic caching: the last block), is a breakpoint; the
prefix up to each breakpoint is written to the cache, and a later request whose prompt starts with a written prefix
reads it. Tokens are counted as characters / 4. It records the markers and `max_tokens` each request carried.

Every answer names the request id the client put in its last message (`rid=...`), the provider and the key, so
the client can tell which target answered and whether it got its own answer. Standard library only; it runs in
a `python:3.11-slim` container in the benchmark so that other gateways in containers can reach it by name.
"""

import json
import re
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
STATE = {"latency_ms": 40, "rules": {}, "log": [], "counters": {}, "cache": {}}
RID = re.compile(r"rid=([A-Za-z0-9_-]+)")


def rule_for(provider, key):
    rules = STATE["rules"]
    return rules.get(f"{provider}:{key}") or rules.get(provider) or {"mode": "ok"}


def count(name):
    with LOCK:
        n = STATE["counters"].get(name, 0) + 1
        STATE["counters"][name] = n
        return n


def last_user_text(body):
    for message in reversed(body.get("messages") or []):
        if message.get("role") == "user":
            content = message.get("content")
            if isinstance(content, list):
                return " ".join(part.get("text", "") for part in content if isinstance(part, dict))
            return str(content or "")
    return ""


def prompt_chars(body):
    total = 0
    for message in body.get("messages") or []:
        content = message.get("content")
        if isinstance(content, list):
            total += sum(len(part.get("text", "")) for part in content if isinstance(part, dict))
        else:
            total += len(str(content or ""))
    return total


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    # Without this, a reused keep-alive connection waits for the client's delayed ACK (about 40 ms on Windows)
    # between the headers and the body, which would be charged to whichever gateway reuses connections.
    disable_nagle_algorithm = True

    def log_message(self, *args):
        pass

    def _send(self, status, payload, headers=None):
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def _read_body(self):
        length = int(self.headers.get("content-length") or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            return json.loads(raw or b"{}")
        except ValueError:
            return {}

    def do_GET(self):
        if self.path == "/_admin/log":
            with LOCK:
                lines = "\n".join(json.dumps(e) for e in STATE["log"])
            data = lines.encode()
            self.send_response(200)
            self.send_header("content-type", "application/x-ndjson")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        parts = self.path.strip("/").split("/")
        if len(parts) >= 3 and parts[1] == "v1" and parts[2] == "models":
            self._send(200, {"object": "list", "data": [
                {"id": m, "object": "model", "owned_by": parts[0]} for m in ("fake-model", "fake-cheap", "fake-strong")]})
            return
        if self.path in ("/", "/health"):
            self._send(200, {"ok": True})
            return
        self._send(404, {"error": {"message": "not found"}})

    def do_POST(self):
        if self.path == "/_admin/config":
            body = self._read_body()
            with LOCK:
                STATE["latency_ms"] = body.get("latency_ms", STATE["latency_ms"])
                STATE["rules"] = body.get("rules", {})
            self._send(200, {"ok": True})
            return
        if self.path == "/_admin/reset":
            with LOCK:
                STATE["log"] = []
                STATE["counters"] = {}
                STATE["cache"] = {}
            self._send(200, {"ok": True})
            return
        parts = self.path.strip("/").split("/")
        if len(parts) == 3 and parts[1:] == ["v1", "messages"]:
            self._messages(parts[0])
            return
        if len(parts) < 4 or parts[1] != "v1" or parts[2:] != ["chat", "completions"]:
            self._send(404, {"error": {"message": f"no route {self.path}"}})
            return
        provider = parts[0]
        auth = self.headers.get("authorization") or ""
        key = auth.split(" ", 1)[1].strip() if " " in auth else (self.headers.get("x-api-key") or "")
        body = self._read_body()
        started = time.time()
        text = last_user_text(body)
        m = RID.search(text)
        rid = m.group(1) if m else None
        stream = bool(body.get("stream"))
        rule = rule_for(provider, key)
        mode = str(rule.get("mode", "ok"))
        n = count(f"{provider}:{key}")
        if mode == "flaky":
            share = float(rule.get("p", 0.3))
            # Deterministic: the n-th request fails when floor(n*p) moved since n-1.
            mode = "500" if int(n * share) != int((n - 1) * share) else "ok"
        entry = {"t": round(started, 4), "provider": provider, "key": key[:24], "model": body.get("model"), "rid": rid,
                 "stream": stream, "mode": mode, "n": n, "prompt_chars": prompt_chars(body),
                 "max_tokens": body.get("max_tokens") or body.get("max_completion_tokens"),
                 "session_headers": {k: v for k, v in self.headers.items()
                                     if "session" in k.lower() or "affinity" in k.lower()}}
        outcome = "?"
        try:
            outcome = self._answer(provider, key, body, rid, stream, mode, rule)
        finally:
            entry["outcome"] = outcome
            entry["ms"] = round((time.time() - started) * 1000, 1)
            with LOCK:
                STATE["log"].append(entry)

    def _messages(self, provider):
        """Anthropic's Messages API with the simulated prompt cache."""
        body = self._read_body()
        started = time.time()
        usage, markers = simulate_cache(provider, body)
        text_in = json.dumps(body.get("messages") or [])
        m = RID.search(text_in[-400:]) or RID.search(text_in)
        rid = m.group(1) if m else None
        entry = {"t": round(started, 4), "provider": provider, "api": "anthropic", "model": body.get("model"),
                 "rid": rid, "stream": bool(body.get("stream")), "max_tokens": body.get("max_tokens"),
                 "markers": markers, "usage": usage, "mode": "ok", "outcome": "ok",
                 "tools": len(body.get("tools") or []), "messages": len(body.get("messages") or []),
                 "top_keys": sorted(k for k in body.keys() if k not in ("messages", "tools", "system"))}
        with LOCK:
            STATE["log"].append(entry)
        out_tokens = 40
        reply = f"ok rid={rid} via={provider}"
        msg_id = f"msg_{provider}_{int(time.time()*1e6)}"
        model = body.get("model") or "fake"
        if not body.get("stream"):
            self._send(200, {"id": msg_id, "type": "message", "role": "assistant", "model": model,
                             "content": [{"type": "text", "text": reply}], "stop_reason": "end_turn",
                             "stop_sequence": None, "usage": {**usage, "output_tokens": out_tokens}})
            return
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("transfer-encoding", "chunked")
        self.end_headers()

        def event(name, obj):
            data = f"event: {name}\ndata: {json.dumps(obj)}\n\n".encode()
            self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
            self.wfile.flush()

        event("message_start", {"type": "message_start", "message": {"id": msg_id, "type": "message", "role": "assistant",
              "model": model, "content": [], "stop_reason": None, "stop_sequence": None,
              "usage": {**usage, "output_tokens": 1}}})
        event("content_block_start", {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}})
        event("content_block_delta", {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": reply}})
        event("content_block_stop", {"type": "content_block_stop", "index": 0})
        event("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": None},
              "usage": {"output_tokens": out_tokens}})
        event("message_stop", {"type": "message_stop"})
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def _error(self, status, message, code=None, kind="api_error", headers=None):
        self._send(status, {"error": {"message": message, "type": kind, "code": code}}, headers)
        return f"http_{status}"

    def _answer(self, provider, key, body, rid, stream, mode, rule):
        latency = STATE["latency_ms"] / 1000.0
        if mode == "hang":
            time.sleep(float(rule.get("seconds", 120)))
            mode = "ok"
        if mode == "drop" or (mode == "midstream" and not stream):
            time.sleep(latency)
            self.close_connection = True
            try:
                self.connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            return "dropped"
        if mode == "429":
            time.sleep(latency / 4)
            headers = {"retry-after": str(rule["retry_after"])} if rule.get("retry_after") is not None else {}
            return self._error(429, "Rate limit reached for requests", "rate_limit_exceeded", "requests", headers)
        if mode == "quota":
            time.sleep(latency / 4)
            return self._error(429, "You exceeded your current quota, please check your plan and billing details.",
                               "insufficient_quota", "insufficient_quota")
        if mode == "402":
            time.sleep(latency / 4)
            return self._error(402, "Insufficient credits. Add more using https://example.invalid/credits", 402)
        if mode == "401":
            time.sleep(latency / 4)
            return self._error(401, "Incorrect API key provided.", "invalid_api_key", "invalid_request_error")
        if mode in ("403", "500", "502", "503", "504"):
            time.sleep(latency / 2)
            return self._error(int(mode), f"upstream failure {mode}", None, "server_error")
        prompt_tokens = max(1, sum(len(json.dumps(m.get("content"))) for m in body.get("messages") or []) // 4)
        reply = f"ok rid={rid} via={provider}/{key[:24]}"
        completion_tokens = 12
        created = int(time.time())
        cid = f"chatcmpl-{provider}-{rid}-{int(time.time()*1e6)}"
        model = body.get("model") or "fake-model"
        usage = {"prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens,
                 "total_tokens": prompt_tokens + completion_tokens}
        time.sleep(latency)
        if not stream:
            self._send(200, {"id": cid, "object": "chat.completion", "created": created, "model": model,
                             "choices": [{"index": 0, "message": {"role": "assistant", "content": reply},
                                          "finish_reason": "stop"}], "usage": usage},
                       {"x-ratelimit-remaining-requests": "999", "x-ratelimit-limit-requests": "1000"})
            return "ok"
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("transfer-encoding", "chunked")
        self.end_headers()

        def chunk(obj):
            data = f"data: {json.dumps(obj) if not isinstance(obj, str) else obj}\n\n".encode()
            self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
            self.wfile.flush()

        words = reply.split(" ")
        base = {"id": cid, "object": "chat.completion.chunk", "created": created, "model": model}
        chunk({**base, "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": None}]})
        for i, word in enumerate(words):
            if mode == "midstream" and i == 2:
                self.close_connection = True
                try:
                    self.connection.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                return "cut_midstream"
            chunk({**base, "choices": [{"index": 0, "delta": {"content": (" " if i else "") + word}, "finish_reason": None}]})
            time.sleep(0.005)
        chunk({**base, "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]})
        if (body.get("stream_options") or {}).get("include_usage"):
            chunk({**base, "choices": [], "usage": usage})
        chunk("[DONE]")
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()
        return "ok"


def tokens(obj):
    return max(1, len(json.dumps(obj, sort_keys=True)) // 4)


def blocks_of(body):
    """The prompt as Anthropic caches it: (block without its marker, marked?) in order: tools, system, messages."""
    out = []

    def add(block):
        if isinstance(block, dict):
            marked = "cache_control" in block
            clean = {k: v for k, v in block.items() if k != "cache_control"}
        else:
            marked, clean = False, block
        out.append((clean, marked))

    for tool in body.get("tools") or []:
        add(tool)
    system = body.get("system")
    if isinstance(system, str):
        add({"type": "text", "text": system})
    elif isinstance(system, list):
        for b in system:
            add(b)
    for m in body.get("messages") or []:
        content = m.get("content")
        if isinstance(content, str):
            add({"role": m.get("role"), "type": "text", "text": content})
        else:
            for b in content or []:
                if isinstance(b, dict):
                    add({"role": m.get("role"), **b})
                else:
                    add(b)
    return out


def simulate_cache(provider, body):
    """Usage as Anthropic's prompt cache would bill it, and the markers seen."""
    import hashlib
    blocks = blocks_of(body)
    cache = STATE["cache"].setdefault(provider, set())
    prefix_hash, prefix_tokens, hashes, sizes = hashlib.sha1(), 0, [], []
    for clean, _ in blocks:
        prefix_hash.update(json.dumps(clean, sort_keys=True).encode())
        prefix_tokens += tokens(clean)
        hashes.append(prefix_hash.hexdigest())
        sizes.append(prefix_tokens)
    total = sizes[-1] if sizes else 0
    breakpoints = [i for i, (_, marked) in enumerate(blocks) if marked]
    if body.get("cache_control") and blocks:
        breakpoints.append(len(blocks) - 1)
    breakpoints = sorted(set(breakpoints))
    read = 0
    if breakpoints:
        last = breakpoints[-1]
        for i in range(last, -1, -1):
            if hashes[i] in cache:
                read = sizes[i]
                break
        written = sizes[last] - read
        for b in breakpoints:
            cache.add(hashes[b])
    else:
        written = 0
    new_input = total - read - written
    markers = {"top_level": bool(body.get("cache_control")), "block_markers": sum(1 for _, m in blocks if m),
               "blocks": len(blocks)}
    return {"input_tokens": new_input, "cache_read_input_tokens": read, "cache_creation_input_tokens": written}, markers


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9900
    server = ThreadingHTTPServer(("0.0.0.0", port), Handler)
    server.daemon_threads = True
    print(f"fake provider on :{port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
