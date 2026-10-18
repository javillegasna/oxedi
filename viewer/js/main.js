// Page wiring: tabs, the file intake and the views.

import { createLoader } from './load.js';
import { renderSummary } from './summary.js';
import { createTables } from './tables.js';
import { createDiagnostics } from './diagnostics.js';
import { createText } from './text.js';
import { createTree } from './tree.js';

const $ = (id) => document.getElementById(id);
const TABS = ['summary', 'tables', 'diagnostics', 'text', 'loops'];

const panels = Object.fromEntries(TABS.map((t) => [t, $(`panel-${t}`)]));
const tabButtons = Object.fromEntries(TABS.map((t) => [t, $(`tab-${t}`)]));
const views = {};
let loaded = null;
let current = 'summary';

/** Shows a tab and tells its view it is visible (virtual lists measure then). */
export function selectTab(name) {
  current = name;
  for (const t of TABS) {
    const on = t === name;
    panels[t].hidden = !on;
    tabButtons[t].setAttribute('aria-selected', String(on));
    tabButtons[t].tabIndex = on ? 0 : -1;
  }
  views[name]?.onShow?.();
}

/** Jumps to a segment of the open file in the Text tab. */
export function showSegment(index, element, component) {
  if (!loaded) return;
  selectTab('text');
  views.text.showSegment(index, element, component);
}

for (const t of TABS) {
  tabButtons[t].addEventListener('click', () => selectTab(t));
  tabButtons[t].addEventListener('keydown', (event) => {
    const at = TABS.indexOf(t);
    const target = {
      ArrowRight: (at + 1) % TABS.length,
      ArrowLeft: (at - 1 + TABS.length) % TABS.length,
      Home: 0,
      End: TABS.length - 1,
    }[event.key];
    if (target === undefined) return;
    const next = TABS[target];
    selectTab(next);
    tabButtons[next].focus();
    event.preventDefault();
  });
}

views.tables = createTables(panels.tables, { onJump: (segment) => showSegment(segment) });
views.diagnostics = createDiagnostics(panels.diagnostics, { onJump: showSegment });
views.text = createText(panels.text);
views.loops = createTree(panels.loops, { onJump: (segment) => showSegment(segment) });

function clearViews() {
  loaded = null;
  $('results').hidden = true;
  panels.summary.textContent = '';
  for (const name of ['tables', 'diagnostics', 'text', 'loops']) views[name].clear();
}

// The tab and filters to put back once the file they belong to is shown again.
let saved = null;

function showLoaded(file) {
  loaded = file;
  renderSummary(panels.summary, file);
  for (const name of ['tables', 'diagnostics', 'text', 'loops']) views[name].show(file);
  $('diag-count').textContent = String(file.view.diagnostics.length);
  $('results').hidden = false;
  const keep = saved && saved.source === file.source ? saved.state : null;
  saved = null;
  if (keep) {
    views.tables.restore(keep.tables);
    views.diagnostics.restore(keep.diagnostics);
  }
  selectTab(keep ? keep.tab : 'summary');
}

/** Remembers the view of the open file, once per file, across quick repeated toggles. */
function remember(source) {
  if (saved && saved.source === source) return;
  const open = loaded && loaded.source === source;
  saved = {
    source,
    state: open ? { tab: current, tables: views.tables.state(), diagnostics: views.diagnostics.state() } : null,
  };
}

function fail(message) {
  clearViews();
  const alert = $('alert');
  alert.textContent = message;
  alert.hidden = false;
}

async function start() {
  let module;
  try {
    module = await import('../pkg/index.js');
  } catch (err) {
    fail(`The WebAssembly module could not be loaded (${err instanceof Error ? err.message : String(err)}). If you build this page yourself, run "make wasm" first.`);
    return;
  }
  $('version').textContent = `v${module.version()}`;

  // One random key per page load: masked values match within a session only.
  const key = crypto.getRandomValues(new Uint8Array(32));
  const mask = $('mask');
  const loader = createLoader({
    parse: module.parse,
    maskKey: () => (mask.checked ? key : undefined),
    status: $('status'),
    alert: $('alert'),
    onLoad: showLoaded,
    onClear: clearViews,
  });

  mask.addEventListener('change', () => {
    $('mask-note').hidden = !mask.checked;
    remember(loader.current());
    loader.reparse();
  });

  const input = $('file');
  input.addEventListener('change', () => {
    const file = input.files?.[0];
    if (file) loader.loadFile(file);
    input.value = '';
  });

  const drop = $('drop');
  const hasFiles = (event) => Array.from(event.dataTransfer?.types ?? []).includes('Files');
  // A drop that misses the zone must not make the browser open the file.
  window.addEventListener('dragover', (event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    drop.classList.add('over');
  });
  window.addEventListener('dragleave', (event) => {
    if (event.target === document.documentElement || event.relatedTarget === null) drop.classList.remove('over');
  });
  window.addEventListener('drop', (event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    drop.classList.remove('over');
    const file = event.dataTransfer.files[0];
    if (file) loader.loadFile(file);
  });

  const sample = $('sample');
  sample.disabled = false;
  sample.addEventListener('click', async () => {
    try {
      const { SAMPLE, SAMPLE_NAME } = await import('../pkg/sample.js');
      loader.loadBytes(SAMPLE_NAME, SAMPLE);
    } catch (err) {
      fail(`The sample could not be loaded (${err instanceof Error ? err.message : String(err)}). If you build this page yourself, run "make wasm" first.`);
    }
  });
}

start();
