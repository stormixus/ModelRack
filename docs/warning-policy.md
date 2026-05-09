# Warning policy

ModelRack keeps a zero Rust-warning baseline. The warning gate is intentionally fail-closed for any new warning bucket or count increase.

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
- `cargo check --message-format=json` reports zero warnings.

Failing outcome:
- any warning bucket appears;
- `cargo check` itself fails.

## Baseline artifact

The committed baseline is `docs/warning-baseline.json`.

Current baseline command: `cargo check --message-format=json`.
Current baseline size: **0 warnings**. Historical warning buckets were removed or made explicit in code: the legacy `objc` macro `cargo-clippy` compatibility feature is declared in `Cargo.toml`, unused migration constants/fields were deleted, test-only utilities compile only for tests, and intentionally retained alternate sort variants use a scoped lint rationale.

## Updating the baseline

Only update the baseline after deliberately fixing warnings or making an intentional scoped lint decision:

```bash
python3 scripts/check-warning-baseline.py --update
python3 scripts/check-warning-baseline.py
```

Before committing an updated baseline:
1. Prefer fixing or deleting the warning source over adding an allow.
2. If an allow is necessary, scope it narrowly and document the rationale at the code site.
3. Run `python3 -m json.tool docs/warning-baseline.json` to validate the artifact.
4. Include the warning gate and `cargo test` evidence in the handoff.

## Release gate interpretation

Plan completion and release readiness both require the warning gate to stay at zero. Any future warning must be fixed, deleted, or converted to a narrow explicit lint rationale before landing.
