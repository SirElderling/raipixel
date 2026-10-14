# Ten proposed agent workflows

All syntax follows [interface v1](../06-agent-interface.md). These are documentation examples, not executed application tests. Save fenced `raipixel` recipes as the named .rai files when implementation exists. Command blocks show actual proposed CLI. Directories are immutable snapshots. Input PNGs must satisfy the stated size/alpha contract; artistic quality and supplied views/poses remain external responsibilities.

## 1. Create a 32×32 fantasy sword

`sword.rai` needs no source art. Separate blade/hilt cels preserve targeted editability. This deliberately simple sword demonstrates controllable structure, not a promised final artistic result.

```text
raipixel 1
palette name=rpg colors=[#252B37,#6A798F,#C7D5DD,#7D452E,#D8A74E]
style name=icon size=[32,32] palette=rpg alpha=binary nonempty=true
asset name=sword size=[32,32] palette=rpg style=icon pivot=[16,28]
layer asset=sword name=blade
layer asset=sword name=hilt
frame asset=sword name=base duration=100
cel target=sword/blade/base
cel target=sword/hilt/base
draw target=sword/blade/base shape=polygon points=[[16,2],[19,7],[18,22],[14,22],[13,7]] color=#252B37
draw target=sword/blade/base shape=polygon points=[[16,4],[17,8],[17,21],[15,21],[15,8]] color=#6A798F
draw target=sword/blade/base shape=line points=[[16,5],[16,20]] color=#C7D5DD
draw target=sword/hilt/base shape=rect rect=[10,21,13,3] color=#D8A74E
draw target=sword/hilt/base shape=rect rect=[15,24,3,5] color=#7D452E
preset name=preview kind=frame target=sword/base scale=8
preset name=game kind=sheet targets=[sword/base] columns=1 padding=1 extrude=1
```

```sh
raipixel run sword.rai --out sword-v1 --export preview
raipixel inspect sword-v1 --asset sword --frame base
raipixel validate sword-v1 --asset sword
raipixel export sword-v1 --preset game --out sword-game
```

Acceptance: dimensions32, five visible palette colors or fewer, binary alpha, two independently editable cels; image/JSON present; pivot [16,28].

## 2. Recolor an existing character

Prerequisite: `hero-v1` contains hero/body/walk0 and hero/body/walk1. Exact RGB mappings preserve per-pixel alpha; no generation.

```text
raipixel 1
replace target=hero/body/walk0 colors=[[#7D452E,#34546E],[#D8A74E,#82AAC3]] preserve-alpha=true
replace target=hero/body/walk1 colors=[[#7D452E,#34546E],[#D8A74E,#82AAC3]] preserve-alpha=true
palette name=blue colors=[#252B37,#34546E,#82AAC3,#C7D5DD]
asset-set name=hero clear=[style] palette=blue
```

```sh
raipixel run recolor.rai --project hero-v1 --out hero-blue
raipixel validate hero-blue --asset hero --checks structure,palette,alpha
```

The supplied palette must include all remaining visible body/equipment colors; otherwise validation fails and the agent adds the intended colors or explicitly quantizes relevant cels. Clearing the old style does not silently retain its previous palette policy. Acceptance: only matched RGBs change; alpha/timing/anchors untouched.

## 3. Modify one layer of a sprite

Prerequisite hero-v1 has hero/armor/walk0. Patch a twelve-pixel armor panel and export the already persisted preview preset if present.

```sh
raipixel op hero-v1 draw target=hero/armor/walk0 shape=rect 'rect=[14,12,4,3]' color=#C7D5DD --out hero-armor-v2
raipixel inspect hero-armor-v2 --asset hero --frame walk0 --layer armor --region 14,12,4,3 --pixels
```

Acceptance: body pixels, all other frames, layer order, timing and anchors unchanged. A flattened PNG needs a supplied armor mask or separated source first; naming a region does not infer armor.

## 4. Create a walking animation from supplied poses

Inputs: 32×32 `inputs/body.png`, and four 32×32 registered `inputs/legs0.png`..`legs3.png`, transparent where absent. The leg poses must depict a believable cycle. Static body plus alternate legs is a supported assembly, not universal character animation.

```text
raipixel 1
asset name=hero size=[32,32] pivot=[16,31]
layer asset=hero name=legs
layer asset=hero name=body
frame asset=hero name=walk0 duration=120
import target=hero/legs/walk0 file="inputs/legs0.png"
import target=hero/body/walk0 file="inputs/body.png"
anchor target=hero/walk0 name=foot at=[16,31]
frame asset=hero name=walk1 source=walk0 duration=120
import target=hero/legs/walk1 file="inputs/legs1.png" replace=true
frame asset=hero name=walk2 source=walk0 duration=120
import target=hero/legs/walk2 file="inputs/legs2.png" replace=true
frame asset=hero name=walk3 source=walk0 duration=120
import target=hero/legs/walk3 file="inputs/legs3.png" replace=true
clip asset=hero name=walk-south frames=[walk0,walk1,walk2,walk3] direction=south playback=forward repeat=0
style name=character size=[32,32] alpha=binary anchors=[foot] nonempty=true
asset-set name=hero style=character
preset name=walk-contact kind=contact targets=[hero/walk0,hero/walk1,hero/walk2,hero/walk3] columns=4 scale=4 clips=[hero/walk-south]
preset name=game kind=sheet targets=[hero/walk0,hero/walk1,hero/walk2,hero/walk3] columns=4 padding=1 clips=[hero/walk-south] components=true
```

```sh
raipixel run walk.rai --out hero-walk --export walk-contact
raipixel validate hero-walk --asset hero --checks structure,animation,registration,alpha
```

Acceptance: four 120ms frames, copied constant foot anchor, unchanged body surfaces, clip cycle metadata; agent visually checks poses. Attacking/casting/death use the same frame model with separately supplied action artwork.

## 5. Assemble eight directional character views

Inputs `views/0-body.png`..`views/7-body.png` and corresponding equipment PNGs are actual separately drawn/generated 32×32 registered views. Index mapping:0 south,1 southwest,2 west,3 northwest,4 north,5 northeast,6 east,7 southeast. d0..d7 are explicit descriptive direction IDs; export consumers use this mapping. No transformation guesses hidden anatomy/equipment.

```text
raipixel 1
palette name=common file="inputs/character.pal"
asset name=hero size=[32,32] palette=common pivot=[16,31]
layer asset=hero name=body
layer asset=hero name=equipment
repeat count=8 var=i
  frame asset=hero name=view-${i} duration=100
  import target=hero/body/view-${i} file="views/${i}-body.png"
  import target=hero/equipment/view-${i} file="views/${i}-equipment.png"
  quantize target=hero/body/view-${i} palette=common alpha=binary threshold=128
  quantize target=hero/equipment/view-${i} palette=common alpha=binary threshold=128
  anchor target=hero/view-${i} name=foot at=[16,31]
  clip asset=hero name=idle-d${i} frames=[view-${i}] direction=d${i}
end
preset name=directions kind=contact targets=[hero/view-0,hero/view-1,hero/view-2,hero/view-3,hero/view-4,hero/view-5,hero/view-6,hero/view-7] columns=4 scale=4
```

```sh
raipixel run directions.rai --out hero-directions --export directions
```

Acceptance: eight supplied views retained, one palette and footline, no inferred back views. View coherence still requires visual/artistic approval.

## 6. Create a terrain tileset and transition

No source images required. Borders are intentionally authored; texture is confined to the interior. A simple transition is constructed from half grass/half dirt, not a general terrain generator.

```text
raipixel 1
palette name=terrain colors=[#4B6B32,#628742,#7D583B,#98704C]
asset name=ground size=[16,16] palette=terrain
layer asset=ground name=base
frame asset=ground name=grass
cel target=ground/base/grass
draw target=ground/base/grass shape=rect rect=[0,0,16,16] color=#4B6B32
texture target=ground/base/grass seed=42 colors=[#628742] density=150 region=[1,1,14,14]
frame asset=ground name=dirt
cel target=ground/base/dirt
draw target=ground/base/dirt shape=rect rect=[0,0,16,16] color=#7D583B
texture target=ground/base/dirt seed=11 colors=[#98704C] density=200 region=[1,1,14,14]
frame asset=ground name=grass-dirt source=grass
draw target=ground/base/grass-dirt shape=rect rect=[8,0,8,16] color=#7D583B
tileset asset=ground size=[16,16] edge-width=1 strip-equality=true
tile target=ground/grass edges=[grass,grass,grass,grass]
tile target=ground/dirt edges=[dirt,dirt,dirt,dirt]
tile target=ground/grass-dirt edges=[mixed,dirt,mixed,grass]
map name=sample asset=ground size=[3,1] cells=[[grass,grass-dirt,dirt]]
preset name=seams kind=seam target=ground/grass
preset name=terrain-game kind=sheet targets=[ground/grass,ground/grass-dirt,ground/dirt] columns=3 padding=1 extrude=1
preset name=map-preview kind=map target=sample scale=4
```

```sh
raipixel run terrain.rai --out terrain-v1 --export seams --export map-preview
raipixel validate terrain-v1 --checks structure,edges,palette
raipixel export terrain-v1 --preset terrain-game --out terrain-game
```

Acceptance: declared grass/dirt/mixed strips compatible, sample adjacency correct, no out-of-palette colors. Smooth transitions and irregular terrain need separately authored tiles/masks. No automatic Wang-set completion.

## 7. Produce twenty controlled weapon variations

Prerequisite sword-v1 from example1. Seeds change blade texture while keeping outline/hilt silhouette and palette. Variation quality/uniqueness is inspected externally.

```text
raipixel 1
repeat count=20 var=i
  asset-copy source=sword name=weapon-${i}
  texture target=weapon-${i}/blade/base seed=${i} colors=[#6A798F,#C7D5DD] density=150 region=[15,8,2,12]
end
preset name=collection kind=sheet targets=[weapon-0/base,weapon-1/base,weapon-2/base,weapon-3/base,weapon-4/base,weapon-5/base,weapon-6/base,weapon-7/base,weapon-8/base,weapon-9/base,weapon-10/base,weapon-11/base,weapon-12/base,weapon-13/base,weapon-14/base,weapon-15/base,weapon-16/base,weapon-17/base,weapon-18/base,weapon-19/base] columns=5 padding=1
```

```sh
raipixel run variants.rai --project sword-v1 --out weapon-family --export collection
```

Acceptance:20 separately editable assets, deterministic seeds0..19, original sword unchanged; all declared style checks pass. Shape variants require explicit draw/transform inputs; random seeds cannot guarantee20 distinct designs.

## 8. Import existing artwork and correct its palette

Input `inputs/generated.png` is supported PNG at 32×32; `inputs/game.pal` is an explicit desired RGB palette. If the generator used a known 4× pixel grid, use a separate import of 128×128 then `transform op=reduce grid=[4,4] phase=[0,0]` before validating; grid estimation is external.

```text
raipixel 1
palette name=game file="inputs/game.pal"
asset name=imported size=[32,32] palette=game
layer asset=imported name=source
frame asset=imported name=base
import target=imported/source/base file="inputs/generated.png"
quantize target=imported/source/base palette=game dither=none alpha=binary threshold=128
preset name=corrected kind=frame target=imported/base scale=4
```

```sh
raipixel run cleanup.rai --out corrected-v1 --export corrected
raipixel inspect corrected-v1 --asset imported --frame base
raipixel validate corrected-v1 --asset imported --checks dimensions,palette,alpha,bounds
```

Correct a known flat background with explicit `alpha mode=key color=#FF00FF match=rgb`, or import a supplied region mask. Acceptance: exact dimensions/allowed colors/binary alpha, meaningful source silhouette requires visual review. Quantization is deterministic cleanup, not detail invention.

## 9. Inspect and validate animation

Prerequisite hero-walk from example4. Persist a diff/onion preset through a recipe; inspection artifacts observe final state and preserve original cels.

```text
raipixel 1
preset name=frame-diff kind=diff targets=[hero/walk0,hero/walk1] scale=4
preset name=neighbors kind=onion asset=hero frames=[walk0,walk1,walk2] scale=4
```

```sh
raipixel run inspect-walk.rai --project hero-walk --out hero-inspected --export frame-diff --export neighbors
raipixel inspect hero-inspected --asset hero --frame walk1
raipixel validate hero-inspected --asset hero --checks structure,animation,registration,alpha,bounds
```

Acceptance: durations/clips/anchors report correctly; known pixel changes match diff stats; no subjective “good walk” score. Intermediate iteration uses a new snapshot boundary.

## 10. Export game-ready spritesheet with metadata

```sh
raipixel export hero-walk --preset game --out hero-game --dry-run
raipixel export hero-walk --preset game --out hero-game
```

Expected sheet:4×(32+2)=136 pixels wide,34 high, content rects at (1,1),(35,1),(69,1),(103,1), all32×32. Clip sequence indices0..3, durations120ms, untrimmed32×32 canvases, pivot [16,31], logical/scaled anchors and component offsets in metadata. Component PNGs preserve layered edit data for consumers; the native snapshot remains the source of truth. Acceptance: consumer draws every frame at a fixed footline and timing using JSON, with no identity inferred from filenames.

## Reuse across independently authored projects

Additional selected operation, not a runtime plugin:

```sh
raipixel op hero-walk import-asset project="sword-v1" source=sword name=held-sword --out hero-with-weapon
```

Imports complete sword layers/frames and needed palette/style values, rejecting unequal name collisions. It does not attach the sword automatically. Use stamp with supplied frame-specific placement, occlusion masks and pre-drawn angled poses where quarter turns are insufficient. The source snapshot stays unchanged.
