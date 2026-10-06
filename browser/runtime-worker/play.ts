import { chooseSourceDirectory, ensureSourceReadPermission } from "../storage/source-directory.js";
import { BrowserAudio } from "./browser-audio.js";
import { BrowserMessageBox } from "./browser-message-box.js";
import { browseSourceDirectory } from "./source-explorer.js";
import { focusSourceEvent, keyboardSourceEvent } from "./keyboard-input.js";
import type { MessageBoxPresentation, RuntimeKeyboardInputEvent, RuntimePointerInputEvent } from "./protocol.js";
import type { InspectExecutableMessage, InspectedExecutableMessage } from "./inspect-worker.js";

const choose = document.querySelector<HTMLButtonElement>("#choose")!;
const back = document.querySelector<HTMLButtonElement>("#back")!;
const launchButton = document.querySelector<HTMLButtonElement>("#launch")!;
const listing = document.querySelector<HTMLUListElement>("#listing")!;
const locationLabel = document.querySelector<HTMLOutputElement>("#location")!;
const status = document.querySelector<HTMLOutputElement>("#status")!;
const worker = new Worker(new URL("./inspect-worker.js", import.meta.url), { type: "module" });
let root: FileSystemDirectoryHandle | null = null;
let segments: string[] = [];
let navigation = 0;
let inspection = 0;
let gameWorker: Worker | null = null;
let gameInputReady = false;
let gameAudio: BrowserAudio | null = null;
let gameMessageBox: BrowserMessageBox | null = null;
let inputSequence = 0;

function sendKeyboardInput(input: RuntimeKeyboardInputEvent): void {
  gameWorker?.postMessage({ type: "keyboard-input", input });
}

window.addEventListener("focus", () => {
  if (gameInputReady && gameWorker !== null && gameMessageBox === null) sendKeyboardInput(focusSourceEvent(++inputSequence, true));
});
window.addEventListener("blur", () => {
  if (gameInputReady && gameWorker !== null) sendKeyboardInput(focusSourceEvent(++inputSequence, false));
});
for (const kind of ["key-down", "key-up"] as const) {
  window.addEventListener(kind === "key-down" ? "keydown" : "keyup", (event) => {
    if (!gameInputReady || gameWorker === null || gameMessageBox !== null || !document.hasFocus()) return;
    const input = keyboardSourceEvent(inputSequence + 1, kind, event.code, event.repeat);
    if (input === null) return;
    inputSequence += 1;
    sendKeyboardInput(input);
    event.preventDefault();
  });
}
type Selection = { root: FileSystemDirectoryHandle; segments: string[]; name: string; sha256: string; storageFingerprint: string };
let pending: Selection | null = null;
let selected: Selection | null = null;

worker.addEventListener("message", (event: MessageEvent<InspectedExecutableMessage>) => {
  const message = event.data;
  if (message.type !== "inspected-executable" || message.requestId !== inspection) return;
  selected = message.result !== null && message.storageFingerprint !== null && pending !== null
    ? { ...pending, sha256: message.result.sha256, storageFingerprint: message.storageFingerprint } : null;
  launchButton.disabled = selected === null;
  status.setAttribute("role", "status");
  status.textContent = message.result === null
    ? `Executable rejected: ${message.error ?? "unknown error"}`
    : selected === null
      ? `Writable storage is unavailable: ${message.error ?? "unknown error"}`
      : `PE32 validated. Ready to launch ${selected.name}.`;
});
worker.addEventListener("error", (event) => showError(new Error(event.message)));

choose.addEventListener("click", async () => {
  const previousStatus = status.textContent;
  const previousRole = status.getAttribute("role");
  choose.disabled = true;
  status.setAttribute("role", "status");
  status.textContent = "Choose a source directory in the picker…";
  try {
    const selected = await chooseSourceDirectory();
    status.textContent = `Opening ${selected.name}…`;
    await ensureSourceReadPermission(selected);
    stopGame();
    root = selected;
    segments = [];
    await render();
  } catch (error) {
    // Cancelling the picker does not discard the previous selection.
    if (error instanceof DOMException && error.name === "AbortError") {
      if (previousRole === null) status.removeAttribute("role");
      else status.setAttribute("role", previousRole);
      status.textContent = root === null ? "Directory selection cancelled." : previousStatus;
    } else showError(error);
  } finally {
    choose.disabled = false;
  }
});
launchButton.addEventListener("click", async () => {
  if (selected === null) return;
  stopGame();
  const running = new Worker(new URL("./game-worker.js", import.meta.url), { type: "module" });
  gameWorker = running;
  const audioReady = BrowserAudio.create(id => {
    if (gameWorker === running) running.postMessage({ type: "audio-ended", resourceId: id });
  }, error => {
    if (gameWorker !== running) return;
    showError(error);
    running.postMessage({ type: "audio-backend-error", detail: error.message });
  });
  running.addEventListener("message", (event: MessageEvent<{
    type: string; detail?: string; requestId?: string; bytes?: ArrayBuffer; resourceId?: string;
    presentation?: MessageBoxPresentation; operation?: string; volume?: number;
  }>) => {
    if (gameWorker !== running) return;
    const message = event.data;
    if (message.type === "message-box") {
      try {
        if (gameMessageBox !== null || typeof message.requestId !== "string" ||
            !/^[1-9][0-9]*$/u.test(message.requestId) || message.presentation === undefined) {
          throw new Error("Invalid game message box operation");
        }
        const requestId = message.requestId;
        // The Chrome dialog takes input focus from the game resource. Rust
        // releases its held state before any dialog keyboard events are consumed.
        sendKeyboardInput(focusSourceEvent(++inputSequence, false));
        if (capturedPointer !== null && canvas.hasPointerCapture(capturedPointer)) {
          canvas.releasePointerCapture(capturedPointer);
        }
        capturedPointer = null;
        heldButtons.clear();
        gameMessageBox = new BrowserMessageBox(message.presentation, result => {
          gameMessageBox = null;
          if (gameWorker === running) {
            canvas.focus({ preventScroll: true });
            sendKeyboardInput(focusSourceEvent(++inputSequence, document.hasFocus()));
            running.postMessage({ type: "message-box-result", requestId, result });
          }
        });
      } catch (error) {
        if (gameMessageBox === null) sendKeyboardInput(focusSourceEvent(++inputSequence, document.hasFocus()));
        running.postMessage({ type: "message-box-failed", detail: error instanceof Error ? error.message : String(error) });
        showError(error);
      }
    } else if (message.type === "audio-command") {
      if (typeof message.requestId !== "string" || !(message.bytes instanceof ArrayBuffer)) {
        showError("Invalid game audio operation"); stopGame(); return;
      }
      const requestId = message.requestId;
      void (gameAudio?.submit(requestId, message.bytes) ?? Promise.resolve(false)).then(accepted => {
        if (gameWorker === running) running.postMessage({ type: "audio-result", requestId, accepted });
      }).catch(error => { showError(error); stopGame(); });
    } else if (message.type === "media-audio-command") {
      if (typeof message.requestId !== "string" || typeof message.resourceId !== "string" ||
          typeof message.operation !== "string" || typeof message.volume !== "number" ||
          (message.bytes !== undefined && !(message.bytes instanceof ArrayBuffer))) {
        showError("Invalid decoded media audio operation"); stopGame(); return;
      }
      const requestId = message.requestId;
      void (gameAudio?.submitMedia(requestId, message.resourceId, message.operation, message.volume, message.bytes) ??
        Promise.resolve(false)).then(accepted => {
        if (gameWorker === running) running.postMessage({ type: "media-audio-result", requestId, accepted });
      }).catch(error => { showError(error); stopGame(); });
    } else if (message.type === "audio-retire" && typeof message.resourceId === "string") {
      gameAudio?.retire(message.resourceId);
    } else if (message.type === "game-error") {
      gameInputReady = false;
      gameMessageBox?.close(); gameMessageBox = null;
      gameAudio?.close(); gameAudio = null; showError(message.detail);
    } else if (message.type === "game-exit") {
      gameInputReady = false;
      gameMessageBox?.close(); gameMessageBox = null;
      gameAudio?.close(); gameAudio = null;
      status.textContent = message.detail ?? "Guest exited";
    } else if (message.type === "game-status") {
      status.textContent = message.detail ?? "";

    }
  });
  running.addEventListener("error", (event) => {
    if (gameWorker === running) {
      // Invalidate this worker before an in-flight audio creation can publish
      // its resource or send the launch message to the failed worker.
      stopGame();
      showError(new Error(event.message));
    }
  });
  launchButton.disabled = true;
  status.setAttribute("role", "status");
  status.textContent = `Launching ${selected.name}…`;
  const canvas = document.createElement("canvas");
  canvas.id = "game-surface";
  canvas.width = 1;
  canvas.height = 1;
  canvas.setAttribute("aria-label", "Game surface");
  canvas.tabIndex = 0;
  let capturedPointer: number | null = null;
  const heldButtons = new Set<"primary" | "secondary">();
  const pointerPosition = (event: PointerEvent, clamp: boolean): { x: number; y: number } | null => {
    const bounds = canvas.getBoundingClientRect();
    if (bounds.width <= 1 || bounds.height <= 1 ||
        !Number.isFinite(event.clientX) || !Number.isFinite(event.clientY) ||
        (!clamp && (event.clientX < bounds.left || event.clientX >= bounds.right ||
          event.clientY < bounds.top || event.clientY >= bounds.bottom))) return null;
    return {
      x: Math.max(0, Math.min(65535, Math.floor((event.clientX - bounds.left) * 65536 / bounds.width))),
      y: Math.max(0, Math.min(65535, Math.floor((event.clientY - bounds.top) * 65536 / bounds.height))),
    };
  };
  const sendPointer = (event: PointerEvent, kind: RuntimePointerInputEvent["kind"]): void => {
    if (!gameInputReady || gameWorker !== running || gameMessageBox !== null || !event.isPrimary || !document.hasFocus()) return;
    const button = kind === "move" ? "none" : event.button === 0 ? "primary" :
      event.button === 2 ? "secondary" : null;
    const position = pointerPosition(event, capturedPointer === event.pointerId);
    if (button === null || position === null) return;
    gameWorker.postMessage({ type: "pointer-input", input: {
      sequence: String(++inputSequence), kind, button, ...position,
    } satisfies RuntimePointerInputEvent });
    if (kind !== "move") event.preventDefault();
  };
  canvas.addEventListener("pointermove", event => sendPointer(event, "move"));
  canvas.addEventListener("pointerdown", event => {
    if (!gameInputReady || gameWorker !== running || gameMessageBox !== null || !event.isPrimary || (event.button !== 0 && event.button !== 2)) return;
    canvas.focus({ preventScroll: true });
    const button = event.button === 0 ? "primary" : "secondary";
    if (pointerPosition(event, false) === null) return;
    sendPointer(event, "down");
    heldButtons.add(button);
    if (event.isTrusted && capturedPointer === null) {
      canvas.setPointerCapture(event.pointerId);
      capturedPointer = event.pointerId;
    }
  });
  canvas.addEventListener("pointerup", event => {
    sendPointer(event, "up");
    if (event.button === 0) heldButtons.delete("primary");
    if (event.button === 2) heldButtons.delete("secondary");
    if (capturedPointer === event.pointerId && heldButtons.size === 0) {
      canvas.releasePointerCapture(event.pointerId);
      capturedPointer = null;
    }
  });
  canvas.addEventListener("pointercancel", event => {
    if (capturedPointer !== event.pointerId) return;
    const position = pointerPosition(event, true);
    if (gameInputReady && position !== null && gameWorker === running && document.hasFocus()) {
      for (const button of heldButtons) {
        gameWorker.postMessage({ type: "pointer-input", input: {
          sequence: String(++inputSequence), kind: "up", button, ...position,
        } satisfies RuntimePointerInputEvent });
      }
    }
    capturedPointer = null;
    heldButtons.clear();
  });
  canvas.addEventListener("contextmenu", event => event.preventDefault());
  document.querySelector("#surface")!.replaceChildren(canvas);
  const offscreen = canvas.transferControlToOffscreen();
  try {
    const audio = await audioReady;
    if (gameWorker !== running) { audio.close(); return; }
    gameAudio = audio;
    running.postMessage({ type: "launch-executable", ...selected, canvas: offscreen }, [offscreen]);
    // Earlier events cannot reach the worker before its launch message. Keep
    // the first emitted input sequence for the initial focus below.
    gameInputReady = true;
  } catch (error) {
    if (gameWorker === running) { showError(error); stopGame(); }
    return;
  }
  sendKeyboardInput(focusSourceEvent(++inputSequence, document.hasFocus()));
});
back.addEventListener("click", () => {
  if (segments.length > 0) {
    segments.pop();
    void render().catch(showError);
  }
});

async function render(): Promise<void> {
  if (root === null) return;
  const request = ++navigation;
  stopGame();
  inspection += 1;
  selected = null;
  pending = null;
  launchButton.disabled = true;
  const current = root;
  const path = [...segments];
  const entries = await browseSourceDirectory(current, path);
  if (request !== navigation) return;
  listing.replaceChildren();
  locationLabel.textContent = [current.name, ...path].join(" / ");
  back.disabled = path.length === 0;
  status.setAttribute("role", "status");
  status.textContent = `${entries.length} folders / executables`;
  for (const entry of entries) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = `${entry.kind === "directory" ? "📁" : entry.kind === "file" ? "📄" : "▶"} ${entry.name}`;
    button.disabled = entry.kind === "file";
    button.addEventListener("click", async () => {
      if (entry.kind === "directory") {
        segments.push(entry.name);
        await render().catch(showError);
      } else if (entry.kind === "executable") {
        try {
          if (request === navigation) {
            stopGame();
            status.textContent = `Validating ${entry.name} as PE32…`;
            selected = null;
            launchButton.disabled = true;
            pending = { root: current, segments: path, name: entry.name, sha256: "", storageFingerprint: "" };
            worker.postMessage({
              type: "inspect-executable",
              requestId: ++inspection,
              root: current,
              segments: path,
              name: entry.name,
            } satisfies InspectExecutableMessage);
          }
        } catch (error) {
          if (request === navigation) showError(error);
        }
      }
    });
    item.append(button);
    listing.append(item);
  }
}

function stopGame(): void {
  gameInputReady = false;
  gameMessageBox?.close();
  gameMessageBox = null;
  gameAudio?.close();
  gameAudio = null;
  gameWorker?.terminate();
  gameWorker = null;
  inputSequence = 0;
  document.querySelector("#surface")!.replaceChildren();
}

function showError(error: unknown): void {
  status.setAttribute("role", "alert");
  status.textContent = error instanceof Error ? error.message : String(error);
}
