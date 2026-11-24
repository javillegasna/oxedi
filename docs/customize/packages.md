# Packages and layers

A spec is written as packages: JSON files, each holding one or more layers of the vocabulary. The
built-in 835 is a stack of them. This page is the reference of the package format: the header,
the layers and the keys each may write, how a stack is put together, what a package may change,
and how to check a stack and run its conformance kit. To patch a built-in spec, and for every key
a spec holds, see [Customizing the spec](spec.md) and its
[Spec keys reference](spec.md#spec-keys-reference).

## What a package is

A package is one JSON object with two keys: `package`, the header, and `spec`, which holds keys of
the spec format, only those its layers own. Nothing else is allowed, and `spec` must be an object.
This is the header of the shipped 5010 guide:

```json
{
  "package": {
    "format": 1,
    "id": "oxedi/guide/005010/835/X221A1",
    "layers": ["guide"],
    "bases": ["oxedi/transaction-set/005010/835"]
  },
  "spec": {"version": {"segment": "GS", "element": 8, "values": ["005010X221A1"]}, "loops": {}}
}
```

The real file holds the guide's loops, segments and balancing rules in `spec` too. The header has
four keys, all required:

| Key | What it holds |
|---|---|
| `format` | The format number the package is written in, an integer from 1 (see [The format number](#the-format-number)). |
| `id` | The package's identity, written in the grammar below. |
| `layers` | The layers the package holds, at least one, each once, in layer order. |
| `bases` | The ids of the packages it builds on, each once; `[]` for a package that builds on none. |

- The id names the package's top layer, the last of `layers`.
- A base listed twice is refused.

### The id

An id is `<author>/<layer>/<release>[/<transaction set>[/<guide>]]`. How many parts it has depends
on the layer:

| Layer | Id form |
|---|---|
| `envelope`, `dictionary` | `<author>/<layer>/<release>` |
| `transaction-set` | `<author>/transaction-set/<release>/<transaction set>` |
| `guide`, `views`, `patch` | `<author>/<layer>/<release>/<transaction set>/<guide>` |

Each part has its own rule:

- author: lowercase ASCII letters, digits and `-`, starting with a letter, at most 32.
- release: six ASCII digits, the X12 release as GS08 starts it (`005010`).
- transaction set: three ASCII digits (`835`).
- guide: uppercase ASCII letters and digits, starting with a letter, at most 16 (`X221A1`).

The text must be exactly that: no other spelling is read. A stack's spec name is its top package's
transaction set, or its release when the id names none, so every built-in stack is named `835`.

These are the shipped packages:

| Id | Layers | Bases |
|---|---|---|
| `oxedi/envelope/005010` | envelope | none |
| `oxedi/dictionary/005010` | dictionary | `oxedi/envelope/005010` |
| `oxedi/transaction-set/005010/835` | transaction-set | `oxedi/dictionary/005010` |
| `oxedi/guide/005010/835/X221A1` | guide | `oxedi/transaction-set/005010/835` |
| `oxedi/views/005010/835/X221A1` | views | `oxedi/guide/005010/835/X221A1` |
| `oxedi/envelope/004010` | envelope | `oxedi/envelope/005010` |
| `oxedi/dictionary/004010` | dictionary | `oxedi/dictionary/005010`, `oxedi/envelope/004010` |
| `oxedi/guide/004010/835/X091A1` | transaction-set, guide | `oxedi/views/005010/835/X221A1`, `oxedi/dictionary/004010` |

The 5010 spec is the stack topped by `oxedi/views/005010/835/X221A1`, and the 4010 spec the stack
topped by `oxedi/guide/004010/835/X091A1`. The edi_835_parser compatibility tables are the patch
package `edi-835-parser/patch/005010/835/X221A1`, built on the 5010 views.
`oxedi.Spec.builtin_packages()` returns the eight texts: the 5010 first, then the three 4010
packages.

## The layers

A layer is one level of a vocabulary. The layers come in this order, and each builds on the ones
before it:

| Layer | What it holds |
|---|---|
| `envelope` | The complete base of a stack: the interchange, group and transaction loops and the envelope segments, and it may write every key the three layers after it own. |
| `dictionary` | Each segment of a release once: its elements and components, their names, types, scale and lengths. |
| `transaction-set` | The loops of one transaction set: their parents, triggers, ends and controls, and each occurrence's segment and position. |
| `guide` | What an implementation guide fixes: usage, maximums, qualifiers, codes and lengths, per occurrence or per segment element; the version; the balancing rules. |
| `views` | The tables, the summaries and the default table. |
| `patch` | A change to any key of the layers below. |

## Who writes which key

A package may write only the keys its layers own. A package of several layers owns what each of
them owns.

| Key | Written by |
|---|---|
| `name` | no layer: a stack takes its name from its top package's id; only a patch may write it |
| `version` | guide |
| `balancing` | guide |
| `loops.<loop>.parent`, `.trigger`, `.end`, `.control` | transaction-set |
| `loops.<loop>.usage`, `.max` | guide |
| `loops.<loop>.occurrences.<name>.segment`, `.pos` | transaction-set |
| `loops.<loop>.occurrences.<name>.usage`, `.max`, `.qualifier`, `.elements` | guide |
| `segments.<id>.elements.<pos>.name`, `.type`, `.scale` | dictionary |
| `segments.<id>.elements.<pos>.codes`, `.min`, `.max`, `.usage` | dictionary and guide |
| `segments.<id>.elements.<pos>.composite.<pos>.*` | as an element's key of the same name |
| `tables`, `summaries`, `default_table` | views |

- The envelope, the base of every stack, may also write every key the dictionary, the transaction set
  and the guide write. It does not write the views' keys.
- A patch writes every key, `name` included.
- The dictionary and the guide both write an element's codes, lengths and usage: the dictionary
  defines the element, and a guide may make it stricter for every occurrence of the segment
  without naming one. `elements` of an occurrence makes it stricter in that occurrence only
  ([the occurrence's `elements`](spec.md#loopsnameoccurrencesname)).
- An empty object (`{}`) or `null` (a deletion) needs the layers of every key beneath it: a guide
  cannot add an empty segment, loop, element or occurrence, nor remove one.
- A key the format does not have is left to the spec's own strict read, which refuses it as an
  unknown key.

A package that writes a key it does not own is refused, and the error says which layer owns it:

```python
import json

import oxedi

packages = oxedi.Spec.builtin_packages()
wrong = json.dumps({
    "package": {"format": 1, "id": "acme/guide/005010/835/X221A1", "layers": ["guide"],
                "bases": ["oxedi/transaction-set/005010/835"]},
    "spec": {"tables": {}},
})
print(oxedi.validate_stack(wrong, packages))
```

```text
no: stack "acme/guide/005010/835/X221A1" does not load: package "acme/guide/005010/835/X221A1" holds tables, a key of layer "views", but its layers are "guide"; move the key to a package of that layer, or list the layer in "layers"
```

## How a stack is put together

The stack of a top package is the package and everything it builds on. A spec is read from it in
these steps:

1. The top package, every package its `bases` name, and theirs, are found among the packages
   given. Each base comes before every package built on it, in the order `bases` lists them, and a
   base shared by two packages comes once. A base not given, two packages with one id, and
   packages that build on each other in a circle are refused.
2. The packages' `spec` values are merged in that order as JSON Merge Patch (RFC 7386): objects
   merge key by key, arrays and other values replace, and `null` deletes.
3. Before a package is merged, its keys are checked against the table above. After it is merged,
   the whole spec so far is read strictly. The spec before that package read cleanly, so a
   refusal is that package's, and the error starts `package "<id>": `.
4. The name comes from the top package's id unless a patch writes `name`, and the spec is compiled
   once, at the end.

So the 4010 stack reads, bases first. The last three are editions of the 5010 packages, so their changes are listed:

```text
oxedi/envelope/005010
oxedi/dictionary/005010
oxedi/transaction-set/005010/835
oxedi/guide/005010/835/X221A1
oxedi/views/005010/835/X221A1
oxedi/envelope/004010
oxedi/dictionary/004010
oxedi/guide/004010/835/X091A1
```

## What a package may change

Every package has a role, which depends on the layers it holds and on the packages it builds on:

| Role | When | What it may do |
|---|---|---|
| narrows | It holds only layers above every direct base's top layer, and shares no layer with any package it builds on. | Add keys, and make stricter what its bases allow. Anything else is refused (`does not only narrow what its bases allow`). |
| edition | It holds a layer that a package it builds on, directly or not, also holds. | Change, add and remove anything; each change is listed. |
| patch | It holds the `patch` layer. | Change, add and remove anything, `name` included; each change is listed. |

The word "narrow" is what the code uses for making a rule stricter. A package that is none of
these (it holds a layer at or below a direct base's top and shares none with its lineage) is
refused.

"Stricter" means, per key:

- fewer codes: a list inside the base's list, or a list where the base had none;
- a larger `min` or a smaller `max` for an element, a component or an occurrence's element entry; a
  smaller `max` for a loop or an occurrence;
- `required` where the base had `situational` (an occurrence's element entry without `usage`
  counts as situational);
- a qualifier's codes inside the base's.

A change of name, type, parent, trigger, end, control, segment or position, of the version, a
balancing rule, a table, a summary, the default table, a qualifier's element or component is a change, not a narrowing, and so is any removal.

The listed changes are `Valid.changes`: each is a `Change` with the package, the path and a
`ChangeKind` (narrowed, widened, changed, removed or added). A value equal to the base's is no
change.

A package built on the shipped 5010 views is always an edition or a patch, since the stack below
it already holds every layer but `patch`. The check lists what it changes instead of
refusing a change that is not stricter. Read the list.

## The format number

- This version of oxedi reads format 1, its newest. A reader reads its own number and every lower
  one.
- A package with a higher number is refused before any other key is read, so a newer key never
  shows up as unknown. The error names both numbers: `package format 2 is newer than 1, the newest
  format this reader takes; read the package with a newer oxedi, or write it in format 1`.
- Every key the format does not have is refused. When oxedi adds an optional key, the number goes
  up by one: packages that do not use the key keep loading, and one that uses it says the new
  number, so an older oxedi refuses it with that text instead of reading it wrongly.
- A kit manifest has its own format number, under the same rule.
- The format is a draft: it can still change until the envelope becomes a base package with its
  own rules (a later step of the roadmap).

## Check a stack and run its kit

A spec is a stack of packages: the shipped 5010 packages and, on top, any package of yours. `oxedi.validate_stack` answers whether a stack loads and what it can do; `oxedi.Spec.builtin_packages()` lists the shipped texts. The answer is a `Valid` or an `Invalid`, and printing either gives the core's text.

```python
(views,) = [t for t in packages if json.loads(t)["package"]["id"] == "oxedi/views/005010/835/X221A1"]
report = oxedi.validate_stack(views, packages)
assert isinstance(report, oxedi.Valid) and report.write.can
print(report)
```

```text
yes: stack "oxedi/views/005010/835/X221A1" loads as spec "835"
packages, bases first: "oxedi/envelope/005010" (narrows), "oxedi/dictionary/005010" (narrows), "oxedi/transaction-set/005010/835" (narrows), "oxedi/guide/005010/835/X221A1" (narrows), "oxedi/views/005010/835/X221A1" (narrows)
can read
can write
can mask
changes: none
```

A kit is a manifest that names inputs and the output each one must give. Python reads no file for it: `oxedi.kit_files` lists the paths it names, you read them, and `oxedi.run_kit` compares. Each case passes, differs (the first differing line of each output) or cannot run (a file it names is missing, an expected output is not text, or the input is not an X12 document). The names a manifest holds are relative to the manifest's folder and may hold `..`: keep them under a root you trust when you read the files.

```python
import pathlib

manifest = json.dumps({
    "kit": {"format": 1, "package": report.id},
    "cases": {"remittance": {
        "input": "remittance.835",
        "expect": {"diagnostics": "remittance.diagnostics.txt"},
    }},
})
files = {
    "remittance.835": pathlib.Path("remittance.835").read_bytes(),
    "remittance.diagnostics.txt": b"",
}
assert oxedi.kit_files(manifest) == ["remittance.835", "remittance.diagnostics.txt"]
result = oxedi.run_kit(manifest, report, files)
assert result.passed
print(result)
```

```text
kit "oxedi/views/005010/835/X221A1": 1 of 1 cases pass
case "remittance": passes
```

The shipped kits are not in the package: they name test fixtures and expected outputs, which the
Python package does not carry. A kit's manifest can sit beside its package.

## Your own spec as a package

A whole spec is the transitional form: one package that holds every layer from envelope to views,
builds on nothing and holds the spec without `name`, since a package that is not a patch may not
write it.

```python
spec = json.loads(oxedi.Spec.builtin().to_json())
del spec["name"]
whole = json.dumps({
    "package": {"format": 1, "id": "acme/views/005010/835/X221A1",
                "layers": ["envelope", "dictionary", "transaction-set", "guide", "views"],
                "bases": []},
    "spec": spec,
})
print(oxedi.validate_stack(whole))
```

```text
yes: stack "acme/views/005010/835/X221A1" loads as spec "835"
packages, bases first: "acme/views/005010/835/X221A1" (narrows)
can read
can write
can mask
changes: none
```

`oxedi.Spec.from_packages(whole)` gives the same spec, and `oxedi.Spec.from_json` still reads a
spec that is not a package.

A patch package on the shipped views lists what it changes:

```python
mine = json.dumps({
    "package": {"format": 1, "id": "acme/patch/005010/835/X221A1", "layers": ["patch"],
                "bases": ["oxedi/views/005010/835/X221A1"]},
    "spec": {"default_table": "services"},
})
report = oxedi.validate_stack(mine, packages)
for change in report.changes:
    print(change)
spec = oxedi.Spec.from_packages(mine, packages)
```

```text
default_table: "claims" becomes "services" (package "acme/patch/005010/835/X221A1")
```

`Spec.patch` merges the same way and lists nothing. A patch package is the form to share, check
and give a kit.

## From the old keys to the new

The spec format changed once. There is no converter: this table gives each old form and what to
write now.

| Before | Now |
|---|---|
| `crates/oxedi_core/specs/835.json`, the whole 5010 spec | five packages under `crates/oxedi_core/specs/oxedi/`, one per layer (the ids above) |
| `crates/oxedi_core/specs/835.4010.json`, a patch of it | three editions: `oxedi/envelope/004010`, `oxedi/dictionary/004010`, `oxedi/guide/004010/835/X091A1` |
| `crates/oxedi_core/specs/edi_835_parser.json`, a patch | the same file, now the patch package `edi-835-parser/patch/005010/835/X221A1` |
| an element's `"required": true` | `"usage": "required"`; drop `"required": false` (`situational` is the default). `required` is refused, true or false |
| an occurrence's `"codes": {"1": ["PR"]}` | `"elements": {"1": {"codes": ["PR"]}}`; the entry may also set `usage`, `min` and `max` |
| an occurrence's codes outside its element's `codes` | refused: an occurrence only makes its element stricter |
| your own whole spec, read with `Spec.from_json` | still read by `Spec.from_json` once the two keys above are rewritten; as a package, the spec without `name` under a header with every layer ([above](#your-own-spec-as-a-package)) |
| your own patch, given to `Spec.patch` | still works; as a package, a `patch` package on `oxedi/views/005010/835/X221A1` |
| Python `oxedi._core.EDI_835_PARSER_PATCH` | `EDI_835_PARSER_PACKAGE`, loaded with `Spec.from_packages(EDI_835_PARSER_PACKAGE, Spec.builtin_packages())`; `oxedi.edi_835_parser.spec()` gives that spec |
| Python `Spec.patch(dict[str, Any])` | `Spec.patch(str \| JsonObject)`; a value JSON cannot hold raises `TypeError` naming its place |
| Rust `Spec::BUILTIN_835_JSON` (the text) | `Builtin::fallback().model()` gives the model (a `Result<SpecModel, _>`), not the text; or `Builtin::packages()` for the package texts |
| Rust `Spec::BUILTIN_835_4010_PATCH` | `Builtin::find("835", Version::V4010)?.loaded()` |
| Rust `Builtin.transaction: &'static str` | a `TransactionSet` (`as_str()`); `Builtin.id` and `Builtin::get(id)` find a built-in by its top package |
| Rust `OccurrenceDef.codes`, `OccurrenceModel.codes` | `OccurrenceDef.elements` (`ElementEntry`), `OccurrenceModel.elements` (`ElementEntryModel`) |
| Rust `ElementModel.required: bool` | `ElementModel.usage: Usage` |
| Rust `OccurrenceError::QualifierInCodes` | its `key` is now `elements.<pos>.codes` (was `codes.<pos>`), and its text reads `<key> sits at the qualifier's own place; its codes are the qualifier's "codes"` |
| load errors that named `codes.<pos>` | they name `elements.<pos>.codes` |
| an occurrence's own code outside its element's codes | refused at load; before, only a qualifier's codes were checked |
| `Spec.to_json()`, Rust `LoadedSpec::to_json` | write `usage` and `elements`, the keys the format now uses |
| a missing-element or length diagnostic | may end with `in occurrence "<name>"` when the occurrence's entry set the rule |
| Rust `OccurrenceError::QualifierCodeOutsideElement` | `OccurrenceError::CodeOutsideElement`, with a new `key` field (`qualifier.codes` or `elements.<pos>.codes`); a qualifier's text is unchanged |

An old key is refused with the keys that are allowed:

```python
try:
    oxedi.Spec.builtin().patch({"segments": {"CLP": {"elements": {"1": {"required": True}}}}})
except oxedi.SpecError as error:
    print(error)
```

```text
applying patch: spec: at segments.CLP.elements.1: unknown field `required`, expected one of `name`, `type`, `scale`, `usage`, `min`, `max`, `codes`, `composite`
```

## Not in the format yet

- No way to name a shared definition: a value two places hold is written twice. The version's
  codes are written in the guide's `version` and in GS08's codes, and equal element definitions
  repeat inside the dictionary and the envelope (`reference_identification` eight times), equal
  code lists repeat inside one guide (the PLB composite six times), and the 4010 guide restates per
  segment the PER codes the 5010 guide sets per occurrence.
- The 5010 dictionary holds what the 835 uses, not the whole release: some lengths are the
  guide's (BPR11 is 9 to 9 where X12 allows 1 to 50), elements the 835 does not use are absent
  (CUR04 to CUR21, QTY03, part of TS3), and element names are the guide's.
- Hierarchical-level (HL) parent pointers are not handled. They are planned as an optional key,
  which raises the format number.
- SQL and the browser viewer use the built-in stacks only; packages load from Python and from the Rust core.
- A package file with a key written twice is read with the last one.
