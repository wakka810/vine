export type RuntimeKeyboardSourceEventKind =
  "focus-gained" | "focus-lost" | "key-down" | "key-up";

export interface RuntimeKeyboardInputEvent {
  sequence: string;
  kind: RuntimeKeyboardSourceEventKind;
  virtualKey: number | null;
  repeat: boolean;
}

export interface RuntimePointerInputEvent {
  sequence: string;
  kind: "move" | "down" | "up";
  button: "none" | "primary" | "secondary";
  x: number; // normalized client coordinate, 0..65535
  y: number;
}

// Rust resolves Windows flags and button identities before requesting a resource.
export interface MessageBoxPresentation {
  title: string;
  text: string;
  buttons: Array<{ id: number; label: string }>;
  defaultButton: number;
  cancelButton: number | null;
  icon: "none" | "error" | "question" | "warning" | "information";
}
