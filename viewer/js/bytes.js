// Byte-level helpers. Offsets from the module index the original bytes; text
// is decoded one piece at a time, never by position in a decoded whole file.

const decoder = new TextDecoder('utf-8', { ignoreBOM: true });

/** Decodes bytes[start, end) as UTF-8; invalid sequences become U+FFFD. */
export function decode(bytes, start, end) {
  return decoder.decode(bytes.subarray(start, end));
}

/** Shows CR and LF as visible glyphs instead of letting them break a line. */
export function visible(text) {
  return text.replace(/\r/g, '␍').replace(/\n/g, '␊');
}

const isTrivia = (b) => b === 0x0d || b === 0x0a || b === 0x20 || b === 0x09;

/**
 * The part of a segment's raw range that holds its elements: leading trivia
 * and the segment terminator (with any trailing line break) are left out.
 * Returns [start, end) as absolute offsets into `bytes`.
 */
export function bodyRange(bytes, segment, terminator) {
  let start = segment.start;
  let end = segment.end;
  while (start < end && isTrivia(bytes[start])) start += 1;
  while (end > start && (bytes[end - 1] === 0x0d || bytes[end - 1] === 0x0a)) end -= 1;
  if (end > start && bytes[end - 1] === terminator) end -= 1;
  return [start, end];
}

/** Splits [start, end) on `separator`; returns the [start, end) of each part. */
export function splitRange(bytes, start, end, separator) {
  const parts = [];
  let from = start;
  for (let i = start; i < end; i += 1) {
    if (bytes[i] === separator) {
      parts.push([from, i]);
      from = i + 1;
    }
  }
  parts.push([from, end]);
  return parts;
}

/**
 * Byte range of an element (1-based; 0 is the segment id) and, when given, of
 * one of its components (1-based). An element with repetitions is cut at the
 * repetition separator first and the first repetition is used (a diagnostic
 * carries no repetition index), so a component never spans the separator.
 * Null when the segment has no such part.
 */
export function elementRange(bytes, view, segmentIndex, element, component) {
  const segment = view.segments[segmentIndex];
  if (!segment || element === undefined || element === null) return null;
  const sep = view.delimiters;
  const [start, end] = bodyRange(bytes, segment, sep.segment.charCodeAt(0));
  const parts = splitRange(bytes, start, end, sep.element.charCodeAt(0));
  const part = parts[element];
  if (!part) return null;
  if (component === undefined || component === null) return part;
  let [from, to] = part;
  if (sep.repetition) [[from, to]] = splitRange(bytes, from, to, sep.repetition.charCodeAt(0));
  const subs = splitRange(bytes, from, to, sep.component.charCodeAt(0));
  return subs[component - 1] ?? null;
}

/** Loop instances holding a segment, outermost first, as `name #ordinal`. */
export function loopPath(view, segmentIndex) {
  const names = [];
  let node = view.segments[segmentIndex]?.node;
  while (node !== undefined && node !== null) {
    const entry = view.nodes[node];
    if (!entry) break;
    if (entry.loop_name !== null) names.push(`${entry.loop_name} #${entry.ordinal}`);
    node = entry.parent;
  }
  return names.reverse();
}
