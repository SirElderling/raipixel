# First executable Foundation slice (single-cel CLI)

Status: implementation on PR #2, not an accepted S1 milestone. This is a
deliberately limited, dependency-free native executable using the existing JSON,
raster and PNG libraries. It is **not** the complete proposed v1 interface.

## Run

```sh
cargo run -- run art.json --out art-v1
cargo run -- run edit.json --project art-v1 --out art-v2
cargo run -- run art.json --out ignored --dry-run
cargo run -- inspect art-v1
cargo run -- inspect art-v1 --query region.json
cargo run -- help
```

The job follows the documented `{"interface":1,"semantics":"1.0","ops":[...]}`
envelope. The first supported workflow has **one asset, one layer, one frame,
one cel**, with a matching canvas and stored cel size. It supports:

- `asset.create` (name, size, optional pivot), `layer.create`, `frame.create`,
  `cel.create`, and `png.import` when the cel does not yet exist.
- `draw` with `pixel`, filled `rect`, or canonical `line`; `grid` with
  literal hexadecimal colors, `@palette/entry` references and optional skip;
  and exact-RGBA flood `fill`.
- `palette.set` with 1–256 ordered named RGB colors (up to 256 palettes),
  optional explicit replacement, and asset palette reference. `palette.map`
  performs simultaneous RGB/RGBA recoloring; `alpha` supports exact color
  keying and thresholding. Palette replacement never silently recolors pixels.
- Single-target `frame` exports with nearest-neighbor scale 1–16.
- `--project` reopening **only** the exact schema subset written by this tool;
  source snapshot paths remain unchanged. The new output contains a native
  `project.json`, `images/000000.png`, and
  `artifacts/EXPORT_NAME/image.png` for requested frame exports.
- `--dry-run` computes the edits and PNG bytes without creating an output.
- `inspect SNAPSHOT` reports bounded project structure. With `--query`, it can
  report exact alpha counts, nontransparent bounds, distinct RGBA count and a
  histogram of the selected cel/frame or image-local region. `pixels:true`
  returns row-major samples for at most 4,096 pixels; `symbols:true` returns
  character rows and a stable RGBA-sorted legend for at most 94 unique colors.
  Histograms are limited to 1–256 entries, sorted by descending count and
  ascending RGBA on ties; truncation is reported. Unsupported comparison
  queries are rejected rather than approximated.

Unknown fields, unsupported job operations, extra assets/layers/frames,
unknown palette references, changed snapshot metadata, and formats outside
this subset fail
explicitly (`E_CAPABILITY` or another typed error). They are **never** silently
discarded or rewritten. Import filenames are resolved under the job directory,
and snapshot image paths are fixed; parent traversal and paths escaping their
base directory are rejected. JSON input is limited to 8 MiB and PNG input
to 128 MiB, with additional image and work ceilings.

PNG samples in the editable cel remain exact RGBA, including RGB under alpha
zero. The snapshot uses the established lossless fixed-Huffman PNG writer,
not the still feature-gated experimental Balanced encoder. Its compression
size is not a claim of minimum-size output.

On ordinary encoding, validation, or file-write errors, output is assembled
inside a same-parent staging directory and removed without publishing the
requested destination. This is **not** yet a proven cross-process no-clobber
publication primitive: `std::fs::rename` does not provide atomic
no-replace semantics on every filesystem. A malicious or concurrent process
creating the destination during rename remains an unresolved race, so the
full Foundation publication guarantee is **not** accepted.

## Tests and missing scope

`tests/foundation_cli.rs`, `tests/cli_color_edit.rs`, `tests/cli_palettes.rs`
and `tests/cli_inspect.rs` exercise new artwork, palette references,
recoloring, alpha cleanup, read-only inspection, hidden transparent RGB,
PNG import, independent decoded pixel expectations, snapshot reopen and
correction without mutating its source, dry-run, unsupported snapshot content,
and failure without destination creation. These tests require only the pinned
Rust toolchain and are invoked by `cargo test --locked --offline`.

Next complete the job/manifest validator and build the general project model,
including multiple layered cels and frame rendering, before claiming scenarios
A–E or full S1 acceptance. Persist and validate unknown-but-supported Core
fields without dropping them. Implement atomic no-clobber publication and
read-only `validate` and remaining inspection comparisons; add broader CLI malformed/path/concurrency
tests and native CI confirmation. Do not treat this executable as production-safe
for adversarial concurrent filesystem mutation.
