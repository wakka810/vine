from __future__ import annotations

import argparse
import json
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

import frida


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Capture a focused Win32 startup trace.")
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=30.0)
    parser.add_argument("--kill-after", action="store_true")
    return parser.parse_args()


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def main() -> int:
    args = parse_args()
    executable = args.exe.resolve()
    if not executable.is_file():
        raise FileNotFoundError(executable)

    agent_path = Path(__file__).with_name("trace-agent.js")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    detached = threading.Event()
    device = frida.get_local_device()

    with args.output.open("w", encoding="utf-8", newline="\n") as output:
        def write_event(event: dict) -> None:
            output.write(json.dumps(event, ensure_ascii=False, separators=(",", ":")) + "\n")
            output.flush()

        write_event({
            "schema": "vine.win32-trace.v1",
            "kind": "trace-start",
            "timestamp": utc_now(),
            "executable": str(executable),
        })

        pid = device.spawn([str(executable)], cwd=str(executable.parent))
        session = None
        resumed = False
        try:
            session = device.attach(pid)
            session.on("detached", lambda *_: detached.set())
            script = session.create_script(agent_path.read_text(encoding="utf-8"))

            def on_message(message: dict, _data: bytes | None) -> None:
                if message["type"] == "send":
                    write_event(message["payload"])
                else:
                    write_event({
                        "schema": "vine.win32-trace.v1",
                        "kind": "agent-message",
                        "timestamp": utc_now(),
                        "message": message,
                    })

            script.on("message", on_message)
            script.load()
            device.resume(pid)
            resumed = True

            try:
                detached.wait(None if args.seconds <= 0 else args.seconds)
            except KeyboardInterrupt:
                pass
        finally:
            if not detached.is_set():
                try:
                    if args.kill_after or not resumed:
                        device.kill(pid)
                    elif session is not None:
                        session.detach()
                except Exception as error:
                    write_event({
                        "schema": "vine.win32-trace.v1",
                        "kind": "cleanup-error",
                        "timestamp": utc_now(),
                        "error": str(error),
                    })
            write_event({
                "schema": "vine.win32-trace.v1",
                "kind": "trace-stop",
                "timestamp": utc_now(),
            })

    print(f"trace written: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"trace failed: {error}", file=sys.stderr)
        raise
