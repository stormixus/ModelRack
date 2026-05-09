# Visual QA masks

ModelRack visual diffs should compare UI content, not unstable capture edges or intentionally variable library content. `scripts/visual-qa-artifacts.py` supports rectangular masks so pixel thresholds can be made actionable without pretending native-window or fixture variance is a pass.

## CLI usage

Inline masks are useful for ad hoc runs:

```bash
python3 scripts/visual-qa-artifacts.py \
  --reference .omx/artifacts/reference.png \
  --current .omx/artifacts/current.png \
  --mask '0,0,1480,24:outer-shadow:native macOS shadow variance' \
  --mask '240,150,620,680:live-library:grid fixture content differs from browser mockup'
```

Each inline mask uses:

```text
x,y,width,height[:name[:reason]]
```

JSON mask files are better for repeatable visual gates:

```json
{
  "masks": [
    {
      "name": "outer-window-shadow",
      "reason": "native macOS shadow and browser reference window chrome differ",
      "rect": { "x": 0, "y": 0, "width": 1480, "height": 32 }
    }
  ]
}
```

Run with:

```bash
python3 scripts/visual-qa-artifacts.py \
  --reference path/to/reference.png \
  --current path/to/current.png \
  --mask-file path/to/masks.json
```

`scripts/capture-smoke.sh` also forwards optional mask configuration:

```bash
MODELRACK_VISUAL_MASK_FILE=path/to/masks.json scripts/capture-smoke.sh
MODELRACK_VISUAL_MASK='0,0,1480,24:outer-shadow:native shadow' scripts/capture-smoke.sh
```

## Report semantics

The `diff/report.json` `comparison` block records:

- `mask_list`: clipped mask rectangles with name, reason, and original coordinates;
- `masked_pixels` and `masked_pixel_ratio`;
- `compared_pixels`;
- `mismatched_pixels`;
- `mismatch_ratio`, calculated only over unmasked pixels.

Masked pixels are shown in blue in `diff/diff.png`. Unmasked mismatches remain magenta.

## Policy

Use masks only for documented non-product variance:

- native outer-window shadow/chrome differences between browser mockups and packaged macOS captures;
- live-library or fixture content that intentionally differs from a static mockup;
- OS-rendering variance explicitly accepted by a design/QA note.

Do not mask regressions in layout, hierarchy, typography, controls, or product behavior. If a mask grows to cover the thing being evaluated, the visual gate should be treated as `mask_required`/not comparable rather than passed.
