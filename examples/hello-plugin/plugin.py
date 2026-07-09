#!/usr/bin/env python3
"""
Minimal termie plugin example — no dependencies.

The plugin protocol is NDJSON over stdin/stdout:

  host -> plugin (one JSON object per line):
    {"t":"hello","api_version":2,"permissions":[...]}
    {"t":"focus_changed","pane":123}
    {"t":"tab_changed","tab":0}
    {"t":"cwd_changed","cwd":"C:/repos/termie"}
    {"t":"bell","pane":123}
    {"t":"widget_clicked","id":"hello"}
    {"t":"message","from":"other-plugin","topic":"say","body":"hi"}
    {"t":"shutdown"}

  plugin -> host (one JSON object per line):
    {"t":"ready","name":"...","api_version":2}
    {"t":"declare_widget","widget":{"id":"...","title":"...","lines":[...]}}
    {"t":"update_widget","widget":{...}}
    {"t":"notify","text":"hi"}
    {"t":"write_pty","data":"ls\r"}           // requires "write_pty" permission
    {"t":"publish","topic":"demo","body":"..."}
    {"t":"subscribe","topic":"*"}

This example:
  - announces itself,
  - declares one text widget,
  - updates it once per second,
  - reacts to focus/tab changes and widget clicks.

Run standalone for quick testing:
    python plugin.py
    -> type {"t":"hello","api_version":2,"permissions":[]}
    -> then e.g. {"t":"focus_changed","pane":1}
"""

from __future__ import annotations

import json
import sys
import threading
import time

WIDGET_ID = "hello"
counter = 0
focused_pane = None
active_tab = 0
lock = threading.Lock()
stop = threading.Event()


def send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def widget_lines() -> list[str]:
    with lock:
        c = counter
        fp = focused_pane
        tab = active_tab
    lines = [f"hello x{c}  (click me)"]
    if fp is not None:
        lines.append(f"focused pane: {fp}")
    lines.append(f"active tab: {tab}")
    return lines


def declare() -> None:
    send(
        {
            "t": "declare_widget",
            "widget": {
                "id": WIDGET_ID,
                "title": "Hello",
                "lines": widget_lines(),
            },
        }
    )


def update() -> None:
    send(
        {
            "t": "update_widget",
            "widget": {
                "id": WIDGET_ID,
                "title": "Hello",
                "lines": widget_lines(),
            },
        }
    )


def ticker() -> None:
    global counter
    while not stop.wait(1.0):
        with lock:
            counter += 1
        update()


def handle_line(line: str) -> None:
    global focused_pane, active_tab
    line = line.strip()
    if not line:
        return
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        return

    t = msg.get("t")
    if t == "hello":
        declare()
        threading.Thread(target=ticker, daemon=True).start()
    elif t == "focus_changed":
        with lock:
            focused_pane = msg.get("pane")
        update()
    elif t == "tab_changed":
        with lock:
            active_tab = int(msg.get("tab", 0))
        update()
    elif t == "widget_clicked":
        if msg.get("id") == WIDGET_ID:
            send({"t": "notify", "text": f"clicked x{counter}"})
    elif t == "shutdown":
        stop.set()
        sys.exit(0)
    # cwd_changed, bell, message, etc. can be handled here as well


def main() -> None:
    # announce before waiting for hello so logs show us early
    send({"t": "ready", "name": "Hello World", "api_version": 2})
    for raw in sys.stdin:
        handle_line(raw)


if __name__ == "__main__":
    main()
