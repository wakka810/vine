import type { PageLifecycleSnapshot, PageLifecycleState } from "./protocol.js";

export interface PageLifecycleObserver {
  snapshot(): PageLifecycleSnapshot;
}

export function observePageLifecycle(
  publish: (lifecycle: PageLifecycleSnapshot) => void,
): PageLifecycleObserver {
  let lifecycle: PageLifecycleSnapshot = {
    state: document.visibilityState === "visible" ? "active" : "frozen",
    generation: 1,
  };

  const update = (state: PageLifecycleState): void => {
    if (lifecycle.state === state) {
      return;
    }
    if (!Number.isSafeInteger(lifecycle.generation + 1)) {
      throw new Error("page lifecycle generation is exhausted");
    }
    lifecycle = {
      state,
      generation: lifecycle.generation + 1,
    };
    publish({ ...lifecycle });
  };

  document.addEventListener("visibilitychange", () => {
    update(document.visibilityState === "visible" ? "active" : "frozen");
  });
  window.addEventListener("pagehide", () => update("frozen"));
  window.addEventListener("pageshow", () => {
    update(document.visibilityState === "visible" ? "active" : "frozen");
  });

  return {
    snapshot: () => ({ ...lifecycle }),
  };
}
