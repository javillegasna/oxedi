// Reading a file, parsing it with the module and reporting progress or errors.

export const CONFIRM_BYTES = 20 * 1024 * 1024;

/** True when the file is large enough that the user should confirm first. */
export function needsConfirmation(file) {
  return file.size > CONFIRM_BYTES;
}

const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve()));

function megabytes(size) {
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * @param {object} options
 * @param {(bytes: Uint8Array, maskKey?: Uint8Array) => string} options.parse  the module's parse
 * @param {() => Uint8Array | undefined} [options.maskKey]  the key to mask patient data with, or nothing to show it
 * @param {HTMLElement} options.status   region that shows "Parsing…"
 * @param {HTMLElement} options.alert    role="alert" region for errors
 * @param {(loaded: {name: string, size: number, bytes: Uint8Array, view: object}) => void} options.onLoad
 * @param {() => void} options.onClear   drops the previous file's views
 */
export function createLoader({ parse, maskKey = () => undefined, status, alert, onLoad, onClear }) {
  let ticket = 0;
  let source = null;

  function fail(message) {
    onClear();
    alert.textContent = message;
    alert.hidden = false;
  }

  // Shows the source, whatever order reads and toggles finish in: the newest run wins.
  async function run(src) {
    const mine = ++ticket;
    alert.hidden = true;
    alert.textContent = '';
    status.textContent = src.reading ? 'Reading\u2026' : 'Parsing\u2026';
    status.hidden = false;
    let bytes;
    try {
      bytes = await src.bytes;
    } catch (err) {
      if (mine === ticket) {
        status.hidden = true;
        fail(`${src.name}: could not be read (${err instanceof Error ? err.message : String(err)})`);
      }
      return;
    }
    if (mine !== ticket) return;
    status.textContent = 'Parsing\u2026';
    await nextFrame();
    if (mine !== ticket) return;
    try {
      const view = JSON.parse(parse(bytes, maskKey()));
      if (mine !== ticket) return;
      onLoad({ name: src.name, size: bytes.length, bytes, view, source: src });
    } catch (err) {
      if (mine === ticket) fail(`${src.name}: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      if (mine === ticket) status.hidden = true;
    }
  }

  function loadBytes(name, bytes) {
    source = { name, bytes: Promise.resolve(bytes), reading: false };
    return run(source);
  }

  async function loadFile(file) {
    if (needsConfirmation(file)) {
      const ok = window.confirm(
        `${file.name} is ${megabytes(file.size)}. Parsing a file this large can take a while and use a lot of memory. Continue?`,
      );
      if (!ok) return;
    }
    const bytes = file.arrayBuffer().then((buffer) => new Uint8Array(buffer));
    source = { name: file.name, bytes, reading: true };
    return run(source);
  }

  /** Parses the newest file chosen again, even one still being read, e.g. after the masking choice changed. */
  function reparse() {
    if (source) run(source);
  }

  /** The newest file chosen, or null: what a result's `source` is compared with. */
  const current = () => source;

  return { loadFile, loadBytes, reparse, current };
}
