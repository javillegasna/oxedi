# Roadmap

What oxedi is working on, why, and where help is welcome. Nothing here is a promise or a date:
plans change with what people need. Work that anyone can take is a GitHub issue: issues labelled
[good first issue](https://github.com/javillegasna/oxedi/labels/good%20first%20issue) have a
Pointers section with where the code lives and which test pins it. Comment on an issue before
starting, so two people do not take the same piece. The
[contributor guide](https://github.com/javillegasna/oxedi/blob/master/CONTRIBUTING.md) says how
to build, test and send a change.

## Working on now: a solid base for more transaction sets

oxedi reads and writes the 835, and its structure already comes from a spec that is plain data.
Before adding other X12 transaction sets, the code underneath is being simplified so that a new
transaction set is a new spec, not new code. None of this changes what oxedi reads or writes:
every expected output stays byte for byte.

- **Thin bindings** (done). Python, DuckDB and the browser viewer no longer repeat rules that
  belong in the Rust core: converting values before writing, picking the spec version of a file,
  turning diagnostics into rows and choosing a masked or unmasked run each live in the core once.
- **One home for X12 values, and a spec the code can build** (in progress). Element types,
  number and date parsing now live in one place. Next, the spec gets a typed model that code can
  build and write back as JSON, which generating specs (next section) needs.
- **Checker and projector with one state per concern** (after that). The structure checker and
  the table projector each track loop instances and match segments to occurrences on their own.
  Sharing that makes a new check, such as hierarchical (HL) parent pointers, a small addition.

## Next: specs for any X12 transaction set

Writing a spec by hand takes hundreds of lines, while the open source
[pyx12](https://github.com/azoner/pyx12) project already maps the 270/271, 276/277, 820, 834, 837
and 999. The plan:

- `oxedi.pyx12.spec_from_map(...)` turns an installed pyx12 map into an oxedi spec, with
  automatic tables (one per loop, one per repeating segment) that a merge patch can rename or
  reshape.
- Patient data columns are flagged from the guide, and masking refuses a spec with no flagged
  column, so a generated spec never seems to hide data it does not.
- A check that hierarchical (HL) parent pointers agree with the nesting.
- Proven first on the 835 (the generated spec must match today's), then the 277, the 271 and
  the 837P.

**Open question, opinions welcome:** should element usage, length and codes be defined once per
segment, as today, or per occurrence, as implementation guides do? Discussed in
[#130](https://github.com/javillegasna/oxedi/issues/130).

## Available now

- **Read an 835** from Python into payments, claims, service lines, adjustments and provider
  adjustments, as tables for Polars, pandas, pyarrow or DuckDB. Every byte of the file is kept,
  and problems come back as diagnostics instead of errors. See [Reading](python/reading.md).
- **Write an 835** from those same tables, with control numbers, counts and balancing checked
  before anything is written. See [Writing](python/writing.md).
- **Use SQL** with the DuckDB extension: read files, folders, globs or cloud storage with
  `read_835`, and write an 835 with `COPY`. See [DuckDB](duckdb/index.md).
- **Validate with pyx12** and get its findings in the same shape as oxedi's own. See
  [Validating with pyx12](python/pyx12.md).
- **Adapt to your payers** by changing the spec. See [The spec](customize/spec.md).
- **Try it in your browser.** Drop an 835 on the [viewer](https://javillegas.dev/oxedi/viewer/);
  the file is read in the browser and never uploaded. See [Try it in your browser](start/viewer.md).
- **Hide patient data** with keyed tokens and coarser dates, in Python and in the viewer, and in
  DuckDB with the next extension version. See
  [Sharing data without patient information](python/without-patient-data.md).
- **Move from edi-835-parser** without rewriting your code. See
  [Migrating from edi-835-parser](migrating-from-edi-835-parser.md).

## Being considered

- **Ask an AI assistant about your remittances**: a local tool server that answers questions
  such as totals by payer or denied claims, over a file, a folder or cloud storage, with patient
  fields masked by default.
- **Matching claims with their payments** once the 837 has a spec.
- **More recipes**, such as Spark and Ray for very large batches.
- **Other languages** beyond Python and DuckDB, such as a JavaScript package or a C library,
  when someone needs them.
- **Very large files** read in pieces instead of all at once.
