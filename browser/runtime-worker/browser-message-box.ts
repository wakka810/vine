import type { MessageBoxPresentation } from "./protocol.js";

export class BrowserMessageBox {
  readonly #dialog = document.createElement("dialog");
  readonly #finish: (result: number) => void;
  #closed = false;

  constructor(presentation: MessageBoxPresentation, finish: (result: number) => void) {
    if (typeof presentation?.title !== "string" || typeof presentation.text !== "string" ||
        presentation.title.length > 4096 || presentation.text.length > 65536 ||
        !Array.isArray(presentation.buttons) || presentation.buttons.length < 1 ||
        presentation.buttons.length > 3 ||
        !["none", "error", "question", "warning", "information"].includes(presentation.icon) ||
        presentation.buttons.some(button => !Number.isInteger(button.id) || button.id < 1 ||
          button.id > 11 || typeof button.label !== "string" || button.label.length > 64) ||
        new Set(presentation.buttons.map(button => button.id)).size !== presentation.buttons.length ||
        !presentation.buttons.some(button => button.id === presentation.defaultButton) ||
        (presentation.cancelButton !== null &&
          !presentation.buttons.some(button => button.id === presentation.cancelButton))) {
      throw new Error("Invalid Rust message box presentation");
    }
    this.#finish = finish;
    this.#dialog.className = "guest-message-box";
    const title = document.createElement("h2");
    title.id = "guest-message-title";
    title.textContent = presentation.title;
    this.#dialog.setAttribute("aria-labelledby", title.id);
    const text = document.createElement("p");
    text.id = "guest-message-text";
    text.textContent = presentation.text;
    this.#dialog.setAttribute("aria-describedby", text.id);
    const icon = document.createElement("span");
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = { none: "", error: "⛔", question: "?", warning: "⚠", information: "ⓘ" }[presentation.icon];
    const buttons = document.createElement("div");
    buttons.className = "guest-message-buttons";
    for (const button of presentation.buttons) {
      const element = document.createElement("button");
      element.type = "button";
      element.textContent = button.label;
      element.autofocus = button.id === presentation.defaultButton;
      element.addEventListener("click", () => this.#complete(button.id));
      buttons.append(element);
    }
    this.#dialog.addEventListener("cancel", event => {
      event.preventDefault();
      if (presentation.cancelButton !== null) this.#complete(presentation.cancelButton);
    });
    this.#dialog.addEventListener("keydown", event => {
      if (event.key === "Enter" && !(event.target instanceof HTMLButtonElement)) {
        event.preventDefault();
        this.#complete(presentation.defaultButton);
      }
    });
    this.#dialog.append(title, icon, text, buttons);
    document.body.append(this.#dialog);
    try { this.#dialog.showModal(); }
    catch (error) { this.close(); throw error; }
  }

  #complete(result: number): void {
    if (this.#closed) return;
    this.close();
    this.#finish(result);
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#dialog.close();
    this.#dialog.remove();
  }
}
