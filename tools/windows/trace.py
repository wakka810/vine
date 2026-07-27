from __future__ import annotations

import argparse
import hashlib
import json
import platform
import sys
import threading
from datetime import datetime, timezone
from pathlib import Path

import frida

TRACE_SCHEMA = "vine.win32-trace.v2"
SUPPORTED_VTABLES = ("idirect3d9", "idirectinput8a")
SUPPORTED_OBSERVATIONS = (
    "startup-info",
    "heap-set-information",
    "heap-lifecycle",
    "heap-allocation",
    "heap-size",
    "module-handle",
    "dynamic-export",
    "tls-lifecycle",
    "fls-lifecycle",
    "pointer-encoding",
    "critical-section",
    "interlocked-increment",
    "interlocked-decrement",
    "get-module-file-name-a",
    "standard-handle",
    "file-type",
    "set-handle-count",
    "command-line-a",
    "environment-strings-w",
    "wide-char-to-multi-byte",
    "multi-byte-to-wide-char",
    "set-current-directory-a",
    "get-file-attributes-a",
    "create-directory-a",
    "source-file-read",
    "create-mutex-a",
    "create-waitable-timer-a",
    "set-waitable-timer",
    "wait-for-single-object-ex",
    "load-icon-a",
    "load-image-a",
    "send-message-a",
    "set-window-pos",
    "set-window-pos-size",
    "set-window-pos-resize",
    "set-window-pos-frame-change-move",
    "load-cursor-a",
    "register-class-ex-a",
    "create-window-ex-a",
    "get-system-menu",
    "get-system-metrics",
    "get-client-rect",
    "get-monitor-info-a",
    "set-timer",
    "create-thread",
    "set-thread-priority",
    "window-dc",
    "screen-dc",
    "font-object",
    "gdiplus-startup",
    "virtual-alloc",
    "ime-context",
    "set-window-long-a",
    "get-string-type-w",
    "lc-map-string-w",
    "get-last-error",
    "set-last-error",
    "get-acp",
    "system-default-lang-id",
    "drive-type",
    "swap-mouse-button",
    "drag-accept-files",
    "time-get-dev-caps",
    "time-get-time",
    "query-performance-frequency",
    "query-performance-counter",
    "get-version-ex-a",
    "get-cp-info",
    "processor-feature",
    "unhandled-exception-filter",
    "get-current-thread",
    "set-thread-affinity-mask",
    "get-system-info",
    "global-memory-status",
    "enum-display-monitors",
    "com-initialization",
    "d3d9-create-device",
    "d3d9-get-surface-level",
    "d3d9-surface-get-dc",
    "d3d9-initial-textures",
    "d3d9-recreated-textures",
    "d3d9-initial-vertex-buffer",
    "d3d9-initial-render-states",
    "d3d9-raster-status",
    "dinput8-root",
    "dinput8-enum-devices",
    "dsound-root",
    "dsound-create-sound-buffer",
    "dsound-buffer-lock-unlock-play",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Capture a focused Win32 startup trace.")
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=30.0)
    parser.add_argument("--kill-after", action="store_true")
    parser.add_argument(
        "--stop-on-event",
        help="Stop capture after the agent emits an event with this kind.",
    )
    parser.add_argument(
        "--force", action="store_true", help="Replace an existing output file."
    )
    parser.add_argument(
        "--vtable",
        action="append",
        choices=SUPPORTED_VTABLES,
        default=[],
        help="Observe one explicitly selected COM vtable (repeatable).",
    )
    parser.add_argument(
        "--observe",
        action="append",
        choices=SUPPORTED_OBSERVATIONS,
        default=[],
        help="Capture one explicitly selected API data contract (repeatable).",
    )
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
    if args.output.exists() and not args.force:
        raise FileExistsError(
            f"trace output already exists (use --force to replace): {args.output}"
        )
    detached = threading.Event()
    stop_requested = threading.Event()
    detached_reason: str | None = None
    detached_crash: str | None = None
    observed_stop_event: str | None = None
    device = frida.get_local_device()
    with executable.open("rb") as target:
        target_sha256 = hashlib.file_digest(target, "sha256").hexdigest()

    output_mode = "w" if args.force else "x"
    write_lock = threading.Lock()
    accepting_agent_events = True
    with args.output.open(output_mode, encoding="utf-8", newline="\n") as output:
        def write_event(event: dict, *, agent_event: bool = False) -> None:
            with write_lock:
                if agent_event and not accepting_agent_events:
                    return
                output.write(
                    json.dumps(event, ensure_ascii=False, separators=(",", ":"))
                    + "\n"
                )
                output.flush()

        write_event(
            {
                "schema": TRACE_SCHEMA,
                "kind": "trace-start",
                "timestamp": utc_now(),
                "executable": str(executable),
                "executableBytes": executable.stat().st_size,
                "executableSha256": target_sha256,
                "python": platform.python_version(),
                "frida": frida.__version__,
                "platform": platform.platform(),
                "arguments": {
                    "seconds": args.seconds,
                    "killAfter": args.kill_after,
                    "stopOnEvent": args.stop_on_event,
                    "vtables": list(dict.fromkeys(args.vtable)),
                    "observations": list(dict.fromkeys(args.observe)),
                },
            },
        )

        pid = device.spawn([str(executable)], cwd=str(executable.parent))
        session = None
        resumed = False
        stop_reason = "setup-failure"
        try:
            session = device.attach(pid)

            def on_detached(reason: str, crash: object | None) -> None:
                nonlocal detached_reason, detached_crash
                detached_reason = reason
                detached_crash = None if crash is None else str(crash)
                detached.set()
                stop_requested.set()

            session.on("detached", on_detached)
            agent_options = {
                "vtables": list(dict.fromkeys(args.vtable)),
                "observations": list(dict.fromkeys(args.observe)),
                "stopOnEvent": args.stop_on_event,
            }
            agent_source = (
                f'"use strict";\n'
                f"globalThis.__vineTraceOptions = {json.dumps(agent_options)};\n"
                + agent_path.read_text(encoding="utf-8")
            )
            script = session.create_script(agent_source)

            def on_message(message: dict, _data: bytes | None) -> None:
                nonlocal observed_stop_event
                if message["type"] == "send":
                    event = message["payload"]
                    write_event(event, agent_event=True)
                    if (
                        args.stop_on_event is not None
                        and event.get("kind") == args.stop_on_event
                    ):
                        observed_stop_event = args.stop_on_event
                        stop_requested.set()
                else:
                    write_event(
                        {
                            "schema": TRACE_SCHEMA,
                            "kind": "agent-message",
                            "timestamp": utc_now(),
                            "message": message,
                        },
                        agent_event=True,
                    )

            script.on("message", on_message)
            script.load()
            device.resume(pid)
            resumed = True

            try:
                timeout = None if args.seconds <= 0 else args.seconds
                if args.stop_on_event is None:
                    ended = detached.wait(timeout)
                else:
                    ended = stop_requested.wait(timeout)
                if detached.is_set():
                    stop_reason = "target-detached"
                elif ended and observed_stop_event is not None:
                    stop_reason = f"event-{observed_stop_event}"
                else:
                    stop_reason = "timeout"
            except KeyboardInterrupt:
                stop_reason = "keyboard-interrupt"
        finally:
            if not detached.is_set():
                try:
                    if args.kill_after or not resumed:
                        device.kill(pid)
                        detached.wait(1.0)
                        stop_reason = f"{stop_reason}-kill"
                    elif session is not None:
                        session.detach()
                        stop_reason = f"{stop_reason}-detach"
                except Exception as error:
                    write_event(
                        {
                            "schema": TRACE_SCHEMA,
                            "kind": "cleanup-error",
                            "timestamp": utc_now(),
                            "error": str(error),
                        }
                    )
            with write_lock:
                accepting_agent_events = False
                output.write(
                    json.dumps(
                        {
                            "schema": TRACE_SCHEMA,
                            "kind": "trace-stop",
                            "timestamp": utc_now(),
                            "reason": stop_reason,
                            "detachedReason": detached_reason,
                            "crash": detached_crash,
                        },
                        ensure_ascii=False,
                        separators=(",", ":"),
                    )
                    + "\n"
                )
                output.flush()

    print(f"trace written: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"trace failed: {error}", file=sys.stderr)
        raise
