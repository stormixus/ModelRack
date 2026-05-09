# Warning policy

ModelRack currently allows a documented Rust warning baseline so stabilization work can distinguish historical noise from newly introduced issues. The policy is intentionally fail-closed for new warning buckets or count increases.

## Enforcement command

Run the warning gate from the repository root:

```bash
python3 scripts/check-warning-baseline.py
```

The script runs:

```bash
cargo check --message-format=json
```

It normalizes local cargo-registry paths, groups warnings by `warning_code|file`, and compares the current counts to `docs/warning-baseline.json`.

Allowed outcome:
- existing classified buckets may stay the same or decrease;
- decreases do not require a baseline update.

Failing outcome:
- a new warning bucket appears without a classification;
- a classified bucket count increases above the baseline;
- `cargo check` itself fails.

## Baseline artifact

The committed baseline is `docs/warning-baseline.json`.

Current baseline command: `cargo check --message-format=json`.
Current baseline size: 193 warnings across eight classified buckets. Each entry records a `disposition`; the current baseline has seven `allowed temporary` buckets and one `false-positive/tool limitation` bucket, with no accepted `must-fix` bucket.

Classified categories:
- upstream `objc` `unexpected_cfgs` macro noise from `objc 0.2.7`;
- intentionally exported macOS app-menu/window hooks pending full menu wiring;
- retained font constants used as bundled-font documentation while Slint consumes registration side effects;
- scanner, string, utility, and view-model compatibility helpers retained for active migration/follow-up surfaces.

## Updating the baseline

Only update the baseline when the warning has been deliberately fixed, accepted, or reclassified:

```bash
python3 scripts/check-warning-baseline.py --update
python3 scripts/check-warning-baseline.py
```

Before committing an updated baseline:
1. Prefer fixing or deleting the warning source over expanding the baseline.
2. Add or revise the bucket disposition/classification in `scripts/check-warning-baseline.py` so future agents know whether the warning is `allowed temporary`, `must-fix`, or `false-positive/tool limitation`.
3. Run `python3 -m json.tool docs/warning-baseline.json` to validate the artifact.
4. Include the warning gate and `cargo test` evidence in the handoff.

## Release gate interpretation

Plan completion allows classified historical warnings only because they are tracked and fail on growth. Release readiness is stricter: each baseline bucket should be either removed, converted to an explicit allow/deny lint policy, or tied to a specific release follow-up before a public release candidate.
