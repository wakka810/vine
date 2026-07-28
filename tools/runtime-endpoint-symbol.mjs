function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function formatRuntimeEndpointSymbol(endpoint) {
  if (!isRecord(endpoint)) {
    throw new TypeError("endpoint must be an object");
  }

  if (typeof endpoint.name === "string" && endpoint.name.length > 0) {
    return endpoint.name;
  }

  const symbol = endpoint.symbol;
  if (isRecord(symbol) && symbol.kind === "name") {
    if (typeof symbol.name === "string" && symbol.name.length > 0) {
      return symbol.name;
    }
    throw new TypeError("named endpoint symbol must have a non-empty name");
  }
  if (isRecord(symbol) && symbol.kind === "ordinal") {
    if (
      Number.isSafeInteger(symbol.ordinal) &&
      symbol.ordinal >= 0 &&
      symbol.ordinal <= 0xffff
    ) {
      return `#${symbol.ordinal}`;
    }
    throw new TypeError("ordinal endpoint symbol must be an unsigned 16-bit integer");
  }

  throw new TypeError("endpoint symbol is missing or unsupported");
}

export function formatManifestRuntimeEndpointIdentity(endpoint) {
  if (!isRecord(endpoint)) {
    throw new TypeError("manifest endpoint must be an object");
  }

  const hasObservedModule = endpoint.observedModule !== undefined;
  const hasObservedSymbol = endpoint.observedSymbol !== undefined;
  if (hasObservedModule || hasObservedSymbol) {
    if (
      typeof endpoint.observedModule !== "string" ||
      endpoint.observedModule.length === 0 ||
      typeof endpoint.observedSymbol !== "string" ||
      endpoint.observedSymbol.length === 0
    ) {
      throw new TypeError(
        "manifest observed endpoint identity requires a non-empty module and symbol",
      );
    }
    return `${endpoint.observedModule}!${endpoint.observedSymbol}`;
  }

  if (typeof endpoint.api === "string" && endpoint.api.length > 0) {
    return endpoint.api;
  }
  throw new TypeError("manifest endpoint API is missing");
}
