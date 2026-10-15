#!/usr/bin/env python3
"""Catch hook requests and write each one down, to see what an agent sends.

Listens on 127.0.0.1:47299 (WAKUWAKU_PORT's default is 47213, so the pet and
this can run side by side). Every POST is written whole to
tools/dumps/<time>-<event>.json: its path (with the query), its headers and
its body (as JSON, or as text when it is not). A line goes to the terminal
for each, and the answer is always 200 with {}, which a hook takes as "no
decision".

    python tools/hook_dump.py [--port 47299]

Point a hook at it, e.g. in settings.json:
    { "type": "http", "url": "http://127.0.0.1:47299/hook?from=dump" }
Standard library only.
"""

import argparse
import json
import re
import sys
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

DUMPS = Path(__file__).resolve().parent / "dumps"

# Where an event's name may be: Claude Code's and Codex's hooks say
# hook_event_name; others may use one of these.
NAME_KEYS = ("hook_event_name", "hookEventName", "event_name", "eventName", "event", "type")


def event_name(body):
    """The event's name from the body, or 'unknown'."""
    if isinstance(body, dict):
        for key in NAME_KEYS:
            value = body.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
    return "unknown"


def safe(name):
    """A name fit for a file name on Windows."""
    name = re.sub(r"[^A-Za-z0-9._-]+", "_", name).strip("._")
    return name[:60] or "unknown"


def dump_path(name):
    """tools/dumps/<time>-<event>.json, never one already there."""
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")[:-3]
    path = DUMPS / f"{stamp}-{safe(name)}.json"
    n = 1
    while path.exists():
        path = DUMPS / f"{stamp}-{safe(name)}-{n}.json"
        n += 1
    return path


class Handler(BaseHTTPRequestHandler):
    server_version = "hook-dump"

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length > 0 else b""
        text = raw.decode("utf-8", errors="replace")
        try:
            body = json.loads(text) if text.strip() else None
            is_json = True
        except json.JSONDecodeError:
            body, is_json = text, False
        name = event_name(body)
        record = {
            "received": datetime.now().isoformat(timespec="milliseconds"),
            "client": f"{self.client_address[0]}:{self.client_address[1]}",
            "path": self.path,
            "headers": dict(self.headers.items()),
            "json": is_json,
            "body": body,
        }
        DUMPS.mkdir(parents=True, exist_ok=True)
        path = dump_path(name)
        path.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8")

        session = body.get("session_id") or body.get("conversationId") if isinstance(body, dict) else None
        tool = body.get("tool_name") if isinstance(body, dict) else None
        parts = [datetime.now().strftime("%H:%M:%S"), name, self.path]
        if session:
            parts.append(f"session={str(session)[:8]}")
        if tool:
            parts.append(f"tool={tool}")
        parts.append(f"{len(raw)} B" + ("" if is_json else " (not JSON)"))
        parts.append(f"-> {path.name}")
        print("  ".join(parts), flush=True)

        answer = b"{}"
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(answer)))
        self.end_headers()
        self.wfile.write(answer)

    def log_message(self, format, *args):
        # The summary line above says it; no access log besides.
        pass


def main():
    parser = argparse.ArgumentParser(description="Write down every hook request sent to it.")
    parser.add_argument("--port", type=int, default=47299)
    args = parser.parse_args()
    try:
        server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    except OSError as e:
        sys.exit(f"hook_dump: cannot listen on 127.0.0.1:{args.port}: {e}")
    print(f"hook_dump: listening on http://127.0.0.1:{args.port}, writing to {DUMPS}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nhook_dump: stopped")
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
