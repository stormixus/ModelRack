#!/usr/bin/env python3
"""Check cargo warning counts against the committed ModelRack baseline.

The baseline is intentionally count-based rather than rendered-text based so that
absolute cargo-registry paths and line-number churn do not make the policy noisy.
The current baseline is zero warnings; any new warning bucket or count increase
fails until it is fixed or explicitly re-baselined with a classification
rationale.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASELINE = ROOT / "docs" / "warning-baseline.json"

WARNING_POLICY = {
    "unexpected_cfgs|$CARGO_REGISTRY/objc-0.2.7/src/macros.rs": {
        "disposition": "false-positive/tool limitation",
        "classification": "upstream objc 0.2 macro expands cfg(cargo-clippy); tracked until objc bridge is replaced or dependency changes",
    },
    "unused_imports|src/macos.rs": {
        "disposition": "allowed temporary",
        "classification": "macOS app-menu/window hooks are intentionally exported ahead of full menu wiring; remove when warning budget is tightened",
    },
    "dead_code|src/fonts.rs": {
        "disposition": "allowed temporary",
        "classification": "font-family constants document bundled font names while Slint currently consumes registration side effects",
    },
    "dead_code|src/macos.rs": {
        "disposition": "allowed temporary",
        "classification": "macOS menu/request hooks are reserved for menu-to-Slint wiring and Dock reopen behavior",
    },
    "dead_code|src/scanner.rs": {
        "disposition": "allowed temporary",
        "classification": "scanner structs keep parsed mesh/progress fields for thumbnail/preview pipeline evolution",
    },
    "dead_code|src/strings.rs": {
        "disposition": "allowed temporary",
        "classification": "legacy string constants remain as i18n extraction anchors during Slint migration",
    },
    "dead_code|src/utils.rs": {
        "disposition": "allowed temporary",
        "classification": "utility helpers retained for planned UI/file-size formatting convergence",
    },
    "dead_code|src/view_model.rs": {
        "disposition": "allowed temporary",
        "classification": "view-model enums/helpers retained for keyboard focus and alternate sort/density surfaces",
    },
}


def normalize_file(path: str) -> str:
    if not path:
        return "<no-span>"
    p = Path(path)
    try:
        return str(p.resolve().relative_to(ROOT))
    except Exception:
        pass
    marker = ".cargo/registry/src/"
    if marker in path:
        tail = path.split(marker, 1)[1].split("/", 1)
        if len(tail) == 2:
            return "$CARGO_REGISTRY/" + tail[1]
    return path


def collect(command: list[str]) -> dict[str, Any]:
    proc = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    buckets: Counter[str] = Counter()
    samples: dict[str, str] = {}
    for line in proc.stdout.splitlines():
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if obj.get("reason") != "compiler-message":
            continue
        message = obj.get("message") or {}
        if message.get("level") != "warning":
            continue
        code = (message.get("code") or {}).get("code") or "no-code"
        spans = message.get("spans") or []
        file_name = normalize_file(spans[0].get("file_name", "<no-span>") if spans else "<no-span>")
        key = f"{code}|{file_name}"
        buckets[key] += 1
        samples.setdefault(key, (message.get("message") or "").splitlines()[0])

    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode)

    entries = []
    for key, count in sorted(buckets.items()):
        code, file_name = key.split("|", 1)
        entries.append(
            {
                "key": key,
                "code": code,
                "file": file_name,
                "count": count,
                "disposition": WARNING_POLICY.get(key, {}).get("disposition", "unclassified"),
                "classification": WARNING_POLICY.get(key, {}).get("classification", "UNCLASSIFIED"),
                "sample": samples.get(key, ""),
            }
        )
    return {
        "schema_version": 1,
        "command": command,
        "total_warnings": sum(buckets.values()),
        "entries": entries,
    }


def load_baseline(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        raise SystemExit(f"missing warning baseline: {path}")


def compare(current: dict[str, Any], baseline: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    base_counts = {entry["key"]: int(entry["count"]) for entry in baseline.get("entries", [])}
    base_classes = {entry["key"]: entry.get("classification", "") for entry in baseline.get("entries", [])}
    for entry in current.get("entries", []):
        key = entry["key"]
        count = int(entry["count"])
        baseline_count = base_counts.get(key, 0)
        classification = base_classes.get(key) or entry.get("classification", "")
        if classification == "UNCLASSIFIED":
            failures.append(f"unclassified warning bucket: {key} ({count})")
        if count > baseline_count:
            failures.append(f"warning bucket increased: {key} baseline={baseline_count} current={count}")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--update", action="store_true", help="write current counts as the baseline")
    parser.add_argument("--command", nargs=argparse.REMAINDER, default=["cargo", "check", "--message-format=json"])
    args = parser.parse_args()
    command = args.command or ["cargo", "check", "--message-format=json"]
    current = collect(command)

    if args.update:
        if any(entry["classification"] == "UNCLASSIFIED" or entry.get("disposition") == "unclassified" for entry in current["entries"]):
            for entry in current["entries"]:
                if entry["classification"] == "UNCLASSIFIED" or entry.get("disposition") == "unclassified":
                    print(f"UNCLASSIFIED: {entry['key']} ({entry['count']})", file=sys.stderr)
            return 2
        args.baseline.parent.mkdir(parents=True, exist_ok=True)
        args.baseline.write_text(json.dumps(current, indent=2) + "\n")
        print(f"updated warning baseline: {args.baseline} ({current['total_warnings']} warnings)")
        return 0

    baseline = load_baseline(args.baseline)
    failures = compare(current, baseline)
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(
        f"warning baseline ok: current={current['total_warnings']} baseline={baseline.get('total_warnings')}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
