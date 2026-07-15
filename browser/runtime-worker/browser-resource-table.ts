export type BrowserResourceId = `browser-resource:${number}`;

const MAX_RUST_U64_ID = 18_446_744_073_709_551_615n;

/** A non-owning association; Rust alone owns COM identity and reference counts. */
export interface LogicalComBinding {
  readonly objectId: string;
  readonly interfaceId: string;
  readonly interfaceName: string;
}

export interface BrowserResourceSnapshot {
  readonly id: BrowserResourceId;
  readonly kind: string;
  readonly binding: Readonly<LogicalComBinding>;
}

export interface BrowserResourceRecord<T> extends BrowserResourceSnapshot {
  readonly resource: T;
}

export class BrowserResourceTable<T> {
  readonly #records = new Map<BrowserResourceId, BrowserResourceRecord<T>>();
  #nextId = 1;

  get size(): number {
    return this.#records.size;
  }

  register(kind: string, binding: LogicalComBinding, resource: T): BrowserResourceRecord<T> {
    invariant(kind.length > 0, "browser resource kind is empty");
    validateLogicalId(binding.objectId, "COM object");
    validateLogicalId(binding.interfaceId, "COM interface");
    invariant(binding.interfaceName.length > 0, "COM interface name is empty");
    invariant(
      Number.isSafeInteger(this.#nextId) && this.#nextId > 0,
      "browser resource ID space is exhausted",
    );
    const id: BrowserResourceId = `browser-resource:${this.#nextId}`;
    this.#nextId += 1;
    const immutableBinding = Object.freeze({ ...binding });
    const record = Object.freeze({ id, kind, binding: immutableBinding, resource });
    this.#records.set(id, record);
    return record;
  }

  get(id: BrowserResourceId): BrowserResourceRecord<T> {
    const record = this.#records.get(id);
    invariant(record !== undefined, `browser resource ${id} is unknown`);
    return record;
  }

  release(id: BrowserResourceId): BrowserResourceRecord<T> {
    const record = this.get(id);
    this.#records.delete(id);
    return record;
  }

  retireLogicalComObject(objectId: string): BrowserResourceRecord<T>[] {
    validateLogicalId(objectId, "COM object");
    const retired: BrowserResourceRecord<T>[] = [];
    for (const [id, record] of this.#records) {
      if (record.binding.objectId !== objectId) {
        continue;
      }
      this.#records.delete(id);
      retired.push(record);
    }
    return retired;
  }

  snapshot(): BrowserResourceSnapshot[] {
    return [...this.#records.values()].map(({ id, kind, binding }) => ({
      id,
      kind,
      binding,
    }));
  }
}

function validateLogicalId(value: string, label: string): void {
  invariant(/^[1-9][0-9]*$/u.test(value), `${label} ID is not a positive decimal identity`);
  invariant(BigInt(value) <= MAX_RUST_U64_ID, `${label} ID exceeds Rust u64 identity space`);
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
