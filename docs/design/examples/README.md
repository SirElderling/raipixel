# Proposed agent workflows

These are documentation contracts, **not executed application tests**. Save each JSON job/query under the stated filename. All operations/fields follow the [interface](../06-agent-interface.md) and [schema](../command-schema.json). Foundation examples use the initial static loop; Core examples require later capabilities. All input artwork prerequisites are explicit. Each output directory must be absent.

## 1. Original artwork from an empty canvas — Foundation

Save as `sword.json`. A compact 12×12 sword uses separate blade/hilt layers and a five-color palette. This supplies actual geometry and shading decisions without source artwork; it is not a prompt-to-image model.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"palette.set","name":"metal","entries":[{"name":"edge","rgb":"#252B37"},{"name":"steel","rgb":"#6A798F"},{"name":"light","rgb":"#C7D5DD"},{"name":"gold","rgb":"#D8A74E"},{"name":"wood","rgb":"#7D452E"}]},
  {"op":"asset.create","name":"sword","size":[12,12],"pivot":[6,11],"palette":"metal"},
  {"op":"layer.create","asset":"sword","name":"blade"},
  {"op":"layer.create","asset":"sword","name":"hilt"},
  {"op":"frame.create","asset":"sword","name":"base"},
  {"op":"cel.create","target":"sword/blade/base"},
  {"op":"cel.create","target":"sword/hilt/base"},
  {"op":"grid","targets":["sword/blade/base"],"at":[4,0],"rows":["..O..",".OLO.",".OSO.",".OSO.",".OSO.",".OSO.",".OOO."],"symbols":{".":"#00000000","O":"@metal/edge","S":"@metal/steel","L":"@metal/light"}},
  {"op":"draw","targets":["sword/hilt/base"],"shape":"rect","rect":[2,7,9,1],"color":"@metal/gold"},
  {"op":"draw","targets":["sword/hilt/base"],"shape":"rect","rect":[5,8,3,3],"color":"@metal/wood"},
  {"op":"draw","targets":["sword/hilt/base"],"shape":"rect","rect":[4,11,5,1],"color":"@metal/gold"}
],"checks":["structure","palette","alpha","bounds"],"exports":[{"name":"preview","kind":"frame","targets":["sword/base"],"scale":16},{"name":"game","kind":"frame","targets":["sword/base"]}]}
```

```sh
raipixel run sword.json --out sword-v1
raipixel inspect sword-v1
raipixel validate sword-v1
```

Review `sword-v1/artifacts/preview/image.png` externally. Pass: recognizable sword reviewed separately, 12×12 game PNG, two editable cels, palette-valid binary alpha, pivot [6,11]. Reopening and editing uses example2.

## 2. Inspect, then change one part — Foundation

Save query as `blade-query.json`, run it, then save job as `recolor.json`. The known steel area uses x=6,y=2..5. Only those selected samples are eligible to change.

```json
{"target":"sword/blade/base","region":[4,0,5,7],"pixels":true,"symbols":true}
```

```sh
raipixel inspect sword-v1 --query blade-query.json
```

```json
{"interface":1,"semantics":"1.0","ops":[{"op":"palette.map","targets":["sword/blade/base"],"region":[6,2,1,4],"pairs":[["#6A798F","#C7D5DD"]]}],"exports":[{"name":"preview","kind":"frame","targets":["sword/base"],"scale":16}]}
```

```sh
raipixel run recolor.json --project sword-v1 --out sword-v2
```

Pass: exactly four steel samples change; hilt, other blade samples, palette, duration and pivot remain identical. Source sword-v1 is unchanged. This is the revision/inspection part of scenario A.

## 3. Import and correct a real PNG — Foundation

`inputs/icon.png` must be a real externally produced 16×16 supported static PNG, with a known incorrect pixel at [7,5]. Save as `correct.json`. Choose replacement through visual/source review; the engine does not infer the defect.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"asset.create","name":"icon","size":[16,16]},
  {"op":"layer.create","asset":"icon","name":"source"},
  {"op":"frame.create","asset":"icon","name":"base"},
  {"op":"png.import","target":"icon/source/base","file":"inputs/icon.png"},
  {"op":"draw","targets":["icon/source/base"],"shape":"pixel","at":[7,5],"color":"#C7D5DD"}
],"exports":[{"name":"corrected","kind":"frame","targets":["icon/base"]}]}
```

```sh
raipixel run correct.json --out icon-corrected
```

Pass: independently decoded exported RGBA equals decoded input at every other position, including hidden RGB; only the declared correction differs. Ancillary/profile/source-file bytes are not the preservation target. To inspect before choosing the correction, first run only the four import operations into a new snapshot, inspect/export it, then run a separate draw job against that snapshot.

## 4. Explicit cleanup and palette extraction — Foundation

Prerequisite icon-corrected. Save as `cleanup.json`. The top four rows are a known key-colored border; avoid whole-image removal of similarly colored details.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"alpha","targets":["icon/source/base"],"kind":"key","color":"#FF00FF","match":"rgb","region":[0,0,16,4]},
  {"op":"palette.extract","source":"icon/source/base","name":"observed","max_colors":256}
],"exports":[{"name":"cleaned","kind":"frame","targets":["icon/base"],"scale":4}]}
```

```sh
raipixel run cleanup.json --project icon-corrected --out icon-clean
```

Pass: only matching alpha in the region changes; hidden RGB is retained; extraction reports exact visible colors. More than 256 colors fails with actionable counts and no publication; inspect the histogram and choose an explicit Core quantization strategy. Extraction does not silently assign a palette or recolor artwork.

## 5. Reusable masked edits — Core

Prerequisite sword-v1. Save as `masked.json`. A hidden mask layer avoids showing the selection image in the final render.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"layer.create","asset":"sword","name":"selection","visible":false},
  {"op":"mask.create","source":"sword/blade/base","target":"sword/selection/base","kind":"color","color":"#6A798F","match":"rgb"},
  {"op":"draw","targets":["sword/blade/base"],"shape":"rect","rect":[0,0,12,12],"color":"@metal/light","mask":"sword/selection/base"}
],"exports":[{"name":"result","kind":"frame","targets":["sword/base"]}]}
```

```sh
raipixel run masked.json --project sword-v1 --out sword-masked
```

Pass: steel-colored blade pixels change; outline, hilt, and all unselected samples/state remain equal. Masks are explicitly constructed predicates, not semantic segmentation.

## 6. Assemble and edit an animation — Core

Save as `motion.json`. This original two-frame glint animation reuses the authored sword; it demonstrates component editing, not an inferred walk cycle.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"frame.create","asset":"sword","name":"glint","source":"base","duration_ms":80},
  {"op":"frame.set","target":"sword/base","duration_ms":160},
  {"op":"draw","targets":["sword/blade/glint"],"shape":"line","points":[[6,2],[6,5]],"color":"@metal/light"},
  {"op":"anchor.set","targets":["sword/base","sword/glint"],"name":"grip","at":[6,9]},
  {"op":"clip.set","asset":"sword","name":"shine","frames":["base","glint"],"playback":"forward","repeat":0},
  {"op":"palette.map","targets":["sword/hilt/base","sword/hilt/glint"],"pairs":[["#7D452E","#6A798F"]]}
],"checks":["structure","animation","registration","palette","bounds"],"exports":[
  {"name":"contact","kind":"contact","targets":["sword/base","sword/glint"],"columns":2,"scale":8},
  {"name":"game","kind":"sheet","targets":["sword/base","sword/glint"],"columns":2,"padding":1,"extrude":1,"clips":["sword/shine"],"components":true}
]}
```

```sh
raipixel run motion.json --project sword-v1 --out sword-motion
raipixel validate sword-motion
```

Pass: two frames at160/80ms; blade edit affects only glint; the named hilt changes across both frames while blades/timing/anchors are preserved by that final map. Sheet is28×14, rects [1,1,12,12] and [15,1,12,12], clip indices [0,1], grip [6,9]. A character walk uses the same model but needs authored/supplied leg/body poses; a sword glint does not prove anatomy generation.

## 7. Import supplied animation poses — Core

`inputs/walk.png` is a 68×34 PNG: two32×32 cells, each with one-pixel border, no extrusion, provided coherent poses. Save as `walk.json`.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"asset.create","name":"hero","size":[32,32],"pivot":[16,31]},
  {"op":"layer.create","asset":"hero","name":"body"},
  {"op":"sheet.import","asset":"hero","layer":"body","file":"inputs/walk.png","size":[32,32],"names":["walk0","walk1"],"durations_ms":[120,120],"columns":2,"padding":1},
  {"op":"anchor.set","targets":["hero/walk0","hero/walk1"],"name":"foot","at":[16,31]},
  {"op":"clip.set","asset":"hero","name":"walk-south","frames":["walk0","walk1"],"direction":"south"}
],"exports":[{"name":"frames","kind":"frames","targets":["hero/walk0","hero/walk1"],"clips":["hero/walk-south"]}]}
```

```sh
raipixel run walk.json --out hero-walk
```

Pass: exact source-cell samples, frame names/timing and foot registration; no inferred directions. Two poses are a minimal assembly fixture, not a claim of a production-quality walk.

## 8. Native component transfer and explicit attachment — Core

Save as `attach.json`. Uses independently produced sword-v1 and hero-walk; attach at a supplied placement in a separate layer. For this fixture at [18,18] keeps the12×12 cel inside the32×32 canvas. Artistic placement/occlusion needs visual approval.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"component.transfer","project":"sword-v1","source":"sword","name":"held"},
  {"op":"layer.create","asset":"hero","name":"equipment"},
  {"op":"cel.create","target":"hero/equipment/walk0"},
  {"op":"cel.create","target":"hero/equipment/walk1"},
  {"op":"stamp","targets":["hero/equipment/walk0","hero/equipment/walk1"],"source":"held/blade/base","at":[18,18]},
  {"op":"stamp","targets":["hero/equipment/walk0","hero/equipment/walk1"],"source":"held/hilt/base","at":[18,18]}
],"exports":[{"name":"equipped","kind":"sheet","targets":["hero/walk0","hero/walk1"],"columns":2,"clips":["hero/walk-south"]}]}
```

```sh
raipixel run attach.json --project hero-walk --out hero-equipped
```

Pass: hero body samples/timing/foot anchors unchanged, imported source intact, equipment separate/editable. Different hand positions require explicit per-frame placements; angled poses beyond exact transforms require authored artwork.

## 9. Controlled variants and seeded procedural detail — Core

Save as `variants.json`; prerequisite sword-v1. A larger family is the same explicit clone/map/noise pattern, with all operations in one invocation; no language loop is needed.

```json
{"interface":1,"semantics":"1.0","ops":[
  {"op":"asset.clone","source":"sword","name":"relic"},
  {"op":"noise","targets":["relic/blade/base"],"seed":"0000000000000000","colors":["@metal/light"],"density":300,"region":[6,2,1,4]},
  {"op":"asset.clone","source":"sword","name":"silver"},
  {"op":"palette.map","targets":["silver/hilt/base"],"pairs":[["#D8A74E","#C7D5DD"]]}
],"exports":[{"name":"family","kind":"sheet","targets":["sword/base","relic/base","silver/base"],"columns":3,"padding":1}]}
```

```sh
raipixel run variants.json --project sword-v1 --out sword-family
```

Pass: same input/seed yields identical state/pixels; unrelated hilt/blade regions preserved per variant; no promise of perceptually distinct random results. Intentional palette/shape changes provide distinguishable variants when randomness alone does not.

## 10. Compare revisions, inspect motion, and export again — Foundation/Core

Save query as `compare.json`; this is a stored-cel sample comparison, not an artistic verdict.

```json
{"target":"sword/blade/base","region":[0,0,12,12],"against":{"project":"sword-v1","target":"sword/blade/base"}}
```

```sh
raipixel inspect sword-v2 --query compare.json
```

Expected changed sample count4. Save the following Core export request as `previews.json`.

```json
{"interface":1,"semantics":"1.0","exports":[
  {"name":"diff","kind":"diff","targets":["sword/base","sword/glint"],"scale":8},
  {"name":"onion","kind":"onion","targets":["sword/base","sword/glint","sword/base"],"scale":8},
  {"name":"game","kind":"sheet","targets":["sword/base","sword/glint"],"columns":2,"padding":1,"extrude":1,"scale":2,"clips":["sword/shine"],"components":true}
]}
```

```sh
raipixel export sword-motion previews.json --out motion-export --dry-run
raipixel export sword-motion previews.json --out motion-export
```

Pass: dry-run creates nothing; real export is56×28 with content rects [2,2,24,24] and [30,2,24,24], pivot [12,22], grip [12,18], logical values retained. Independent consumer reads metadata, not filenames. Core reproduces examples from the same inputs into fresh destinations; byte comparisons exclude only absolute destination strings in process reports.
