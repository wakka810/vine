export function browserDelayMilliseconds(service: string, words: readonly number[]): number {
  const audio = service === "browser.audio-delay";
  if ((!audio && service !== "browser.timer-delay" && service !== "browser.sleep" &&
      service !== "browser.object-wait" && service !== "browser.message-wait") ||
      words.some(word => !Number.isInteger(word) || word < 0 || word > 0xffff_ffff)) {
    throw new Error("Invalid browser timer request");
  }
  const milliseconds = words[audio ? 1 : 0];
  if (!Number.isInteger(milliseconds) || milliseconds! < 1 || milliseconds! > 0x7fff_ffff) {
    throw new Error("Invalid browser timer delay");
  }
  if (audio) {
    const active = words[0] === 1;
    const count = words[active ? 5 : 3];
    const timeout = words[active ? 4 : 2];
    // Rust captures the handles and actual host start time. A retry can shorten
    // the delay while retaining the original finite wait deadline.
    if ((words[0] !== 1 && words[0] !== 2) || !Number.isInteger(count) || count! < 1 || count! > 32 ||
        words.length !== (active ? 8 : 6) + count! || !Number.isInteger(timeout) ||
        timeout! < 1 || timeout! > 100 || milliseconds! > timeout!) {
      throw new Error("Invalid browser audio wait");
    }
  } else if (words.length !== 1) {
    throw new Error("Invalid browser timer arguments");
  }
  return milliseconds!;
}
