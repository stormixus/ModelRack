#!/usr/bin/env python3
"""Create deterministic visual QA artifact reports for ModelRack screenshots.

The script intentionally avoids third-party Python dependencies so it can run from a
fresh checkout. It supports non-interlaced 8-bit PNG images, writes reference/current/
diff report.json files, detects blank captures, and fails closed before diff thresholds
when either side is invalid.
"""
from __future__ import annotations

import argparse
import binascii
import datetime as dt
import hashlib
import json
import math
import os
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Any

FAILURE_REASONS = {
    "none",
    "blank_reference",
    "blank_current",
    "capture_failed",
    "diff_threshold_exceeded",
    "mask_required",
    "missing_reference",
    "environment_mismatch",
}

PNG_SIG = b"\x89PNG\r\n\x1a\n"


def utc_now() -> str:
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def git_commit(root: Path) -> str | None:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, stderr=subprocess.DEVNULL, text=True
        ).strip()
    except Exception:
        return None


def app_version(root: Path) -> str | None:
    cargo = root / "Cargo.toml"
    if not cargo.exists():
        return None
    for line in cargo.read_text().splitlines():
        if line.strip().startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    return None


def sha256(path: Path) -> str | None:
    if not path.exists() or not path.is_file():
        return None
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def read_png(path: Path) -> tuple[int, int, list[tuple[int, int, int, int]]]:
    data = path.read_bytes()
    if not data.startswith(PNG_SIG):
        raise ValueError("not a PNG file")
    pos = len(PNG_SIG)
    width = height = color_type = bit_depth = interlace = None
    idat = bytearray()
    while pos < len(data):
        if pos + 8 > len(data):
            raise ValueError("truncated PNG chunk")
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _compression, _filter, interlace = struct.unpack(
                ">IIBBBBB", chunk
            )
        elif kind == b"IDAT":
            idat.extend(chunk)
        elif kind == b"IEND":
            break
    if width is None or height is None:
        raise ValueError("missing IHDR")
    if bit_depth != 8 or interlace != 0:
        raise ValueError("only 8-bit non-interlaced PNG is supported")
    channels = {0: 1, 2: 3, 4: 2, 6: 4}.get(color_type)
    if channels is None:
        raise ValueError(f"unsupported PNG color type: {color_type}")
    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    rows: list[bytes] = []
    prev = bytes(stride)
    i = 0
    for _y in range(height):
        filt = raw[i]
        i += 1
        scan = bytearray(raw[i : i + stride])
        i += stride
        recon = bytearray(stride)
        for x in range(stride):
            left = recon[x - channels] if x >= channels else 0
            up = prev[x]
            up_left = prev[x - channels] if x >= channels else 0
            val = scan[x]
            if filt == 0:
                out = val
            elif filt == 1:
                out = val + left
            elif filt == 2:
                out = val + up
            elif filt == 3:
                out = val + ((left + up) // 2)
            elif filt == 4:
                p = left + up - up_left
                pa = abs(p - left)
                pb = abs(p - up)
                pc = abs(p - up_left)
                pred = left if pa <= pb and pa <= pc else up if pb <= pc else up_left
                out = val + pred
            else:
                raise ValueError(f"unsupported PNG filter: {filt}")
            recon[x] = out & 0xFF
        prev = bytes(recon)
        rows.append(prev)
    pixels: list[tuple[int, int, int, int]] = []
    for row in rows:
        for x in range(0, len(row), channels):
            if color_type == 0:
                g = row[x]
                pixels.append((g, g, g, 255))
            elif color_type == 2:
                pixels.append((row[x], row[x + 1], row[x + 2], 255))
            elif color_type == 4:
                g, a = row[x], row[x + 1]
                pixels.append((g, g, g, a))
            elif color_type == 6:
                pixels.append((row[x], row[x + 1], row[x + 2], row[x + 3]))
    return width, height, pixels


def write_png_rgba(path: Path, width: int, height: int, pixels: list[tuple[int, int, int, int]]) -> None:
    def chunk(kind: bytes, payload: bytes) -> bytes:
        crc = binascii.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", crc)

    raw = bytearray()
    for y in range(height):
        raw.append(0)
        row = pixels[y * width : (y + 1) * width]
        for r, g, b, a in row:
            raw.extend([r, g, b, a])
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.write_bytes(PNG_SIG + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b""))


def blank_detection(path: Path) -> tuple[dict[str, Any], int | None, int | None, list[tuple[int, int, int, int]] | None, str | None]:
    if not path.exists() or path.stat().st_size == 0:
        return blank_result(True, "missing_or_empty"), None, None, None, "capture_failed"
    try:
        width, height, pixels = read_png(path)
    except Exception as e:
        return blank_result(True, f"unreadable_png:{e}"), None, None, None, "capture_failed"
    lumas = [(0.2126 * r + 0.7152 * g + 0.0722 * b) * (a / 255.0) for r, g, b, a in pixels]
    mean = sum(lumas) / len(lumas) if lumas else 0.0
    variance = sum((l - mean) ** 2 for l in lumas) / len(lumas) if lumas else 0.0
    stddev = math.sqrt(variance)
    non_empty = sum(1 for l in lumas if l > 3.0) / len(lumas) if lumas else 0.0
    is_blank = stddev < 0.5
    reason = "low_luma_variance" if is_blank else None
    return {
        "is_blank": is_blank,
        "mean_luma": round(mean, 4),
        "stddev_luma": round(stddev, 4),
        "non_empty_pixel_ratio": round(non_empty, 6),
        "reason": reason,
    }, width, height, pixels, None


def blank_result(is_blank: bool, reason: str | None) -> dict[str, Any]:
    return {
        "is_blank": is_blank,
        "mean_luma": 0.0,
        "stddev_luma": 0.0,
        "non_empty_pixel_ratio": 0.0,
        "reason": reason,
    }


def copy_image(src: Path | None, dest_dir: Path, fallback_name: str) -> Path | None:
    if src is None or not src.exists() or not src.is_file():
        return None
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / fallback_name
    shutil.copy2(src, dest)
    return dest


def report(
    *,
    root: Path,
    artifact_kind: str,
    image_path: Path | None,
    source_path: str | None,
    source_command: str | None,
    failure_reason: str,
    blank: dict[str, Any] | None,
    width: int | None,
    height: int | None,
    reference_path: str | None = None,
    current_path: str | None = None,
    diff_path: str | None = None,
    threshold: float = 0.01,
    result: str = "not_compared",
    masks: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    if failure_reason not in FAILURE_REASONS:
        raise ValueError(f"unknown failure_reason: {failure_reason}")
    git = git_commit(root)
    return {
        "schema_version": 1,
        "artifact_kind": artifact_kind,
        "created_at_utc": utc_now(),
        "failure_reason": failure_reason,
        "source": {
            "path": source_path,
            "command": source_command,
            "git_commit": git,
        },
        "provenance": {
            "app_version": app_version(root),
            "app_build_id": git,
            "git_commit": git,
        },
        "environment": {
            "platform": sys.platform,
            "backend": "artifact-analyzer",
            "os_version": platform.platform(),
            "scale_factor": None,
            "viewport": {"width": width, "height": height},
        },
        "image": {
            "path": str(image_path) if image_path else None,
            "width": width,
            "height": height,
            "sha256": sha256(image_path) if image_path else None,
            "blank_detection": blank or blank_result(False, None),
        },
        "comparison": {
            "reference_path": reference_path,
            "current_path": current_path,
            "diff_path": diff_path,
            "mask_list": masks or [],
            "threshold": {"allowed_mismatch_ratio": threshold},
            "result": result,
        },
    }


def parse_mask_spec(spec: str) -> dict[str, Any]:
    """Parse x,y,width,height[:name[:reason]] mask specs.

    The compact CLI form keeps capture commands readable while the JSON form below
    supports richer checked-in mask files.
    """
    coords, *meta = spec.split(":")
    parts = [part.strip() for part in coords.split(",")]
    if len(parts) != 4:
        raise ValueError(f"mask must be x,y,width,height[:name[:reason]]: {spec}")
    try:
        x, y, width, height = (int(part) for part in parts)
    except ValueError as exc:
        raise ValueError(f"mask coordinates must be integers: {spec}") from exc
    return {
        "name": meta[0].strip() if meta and meta[0].strip() else f"rect-{x}-{y}-{width}-{height}",
        "reason": meta[1].strip() if len(meta) > 1 and meta[1].strip() else "unspecified",
        "x": x,
        "y": y,
        "width": width,
        "height": height,
    }


def read_mask_file(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text())
    entries = data.get("masks", data) if isinstance(data, dict) else data
    if not isinstance(entries, list):
        raise ValueError("mask file must be a list or an object with a masks list")
    masks: list[dict[str, Any]] = []
    for idx, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise ValueError(f"mask entry {idx} must be an object")
        rect = entry.get("rect", entry)
        try:
            x = int(rect["x"])
            y = int(rect["y"])
            width = int(rect["width"])
            height = int(rect["height"])
        except Exception as exc:
            raise ValueError(f"mask entry {idx} requires integer x/y/width/height") from exc
        masks.append(
            {
                "name": str(entry.get("name") or entry.get("label") or f"mask-{idx + 1}"),
                "reason": str(entry.get("reason") or "unspecified"),
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }
        )
    return masks


def load_masks(mask_specs: list[str], mask_files: list[str]) -> list[dict[str, Any]]:
    masks: list[dict[str, Any]] = []
    for mask_file in mask_files:
        masks.extend(read_mask_file(Path(mask_file)))
    for spec in mask_specs:
        masks.append(parse_mask_spec(spec))
    return masks


def normalize_masks(masks: list[dict[str, Any]], width: int, height: int) -> list[dict[str, Any]]:
    normalized: list[dict[str, Any]] = []
    for mask in masks:
        x = max(0, int(mask["x"]))
        y = max(0, int(mask["y"]))
        right = min(width, int(mask["x"]) + max(0, int(mask["width"])))
        bottom = min(height, int(mask["y"]) + max(0, int(mask["height"])))
        clipped_width = max(0, right - x)
        clipped_height = max(0, bottom - y)
        if clipped_width == 0 or clipped_height == 0:
            continue
        normalized.append(
            {
                "name": str(mask.get("name") or "unnamed-mask"),
                "reason": str(mask.get("reason") or "unspecified"),
                "shape": "rect",
                "x": x,
                "y": y,
                "width": clipped_width,
                "height": clipped_height,
                "original": {
                    "x": int(mask["x"]),
                    "y": int(mask["y"]),
                    "width": int(mask["width"]),
                    "height": int(mask["height"]),
                },
            }
        )
    return normalized


def masked_pixels(width: int, height: int, masks: list[dict[str, Any]]) -> set[int]:
    masked: set[int] = set()
    for mask in masks:
        for y in range(mask["y"], mask["y"] + mask["height"]):
            row = y * width
            for x in range(mask["x"], mask["x"] + mask["width"]):
                masked.add(row + x)
    return masked


def compare_pixels(
    reference: list[tuple[int, int, int, int]],
    current: list[tuple[int, int, int, int]],
    threshold: float,
    *,
    width: int,
    height: int,
    masks: list[dict[str, Any]],
) -> tuple[float, str, list[tuple[int, int, int, int]], dict[str, int | float]]:
    mismatches = 0
    diff: list[tuple[int, int, int, int]] = []
    masked = masked_pixels(width, height, masks)
    compared = 0
    for idx, (rp, cp) in enumerate(zip(reference, current)):
        if idx in masked:
            diff.append((56, 112, 255, 120))
            continue
        compared += 1
        delta = max(abs(rp[0] - cp[0]), abs(rp[1] - cp[1]), abs(rp[2] - cp[2]), abs(rp[3] - cp[3]))
        if delta > 12:
            mismatches += 1
            diff.append((255, 0, 64, 255))
        else:
            grey = int((cp[0] + cp[1] + cp[2]) / 3)
            diff.append((grey, grey, grey, 80))
    ratio = mismatches / compared if compared else 0.0
    stats = {
        "mismatched_pixels": mismatches,
        "compared_pixels": compared,
        "masked_pixels": len(masked),
        "masked_pixel_ratio": round(len(masked) / len(reference), 8) if reference else 0.0,
    }
    return ratio, "pass" if ratio <= threshold else "fail", diff, stats


def run(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    run_id = args.run_id or dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    run_dir = Path(args.out_dir).resolve() / run_id
    reference_dir = run_dir / "reference"
    current_dir = run_dir / "current"
    diff_dir = run_dir / "diff"
    reference_src = Path(args.reference).resolve() if args.reference else None
    current_src = Path(args.current).resolve() if args.current else None
    threshold = float(args.threshold)
    raw_masks = load_masks(args.mask or [], args.mask_file or [])

    reference_img = copy_image(reference_src, reference_dir, "reference.png")
    current_img = copy_image(current_src, current_dir, "current.png")

    ref_blank, ref_w, ref_h, ref_pixels, ref_error = blank_detection(reference_img) if reference_img else (blank_result(True, "missing_reference"), None, None, None, "missing_reference")
    cur_blank, cur_w, cur_h, cur_pixels, cur_error = blank_detection(current_img) if current_img else (blank_result(True, "missing_or_empty"), None, None, None, "capture_failed")

    ref_failure = "blank_reference" if ref_blank.get("is_blank") else ref_error or "none"
    cur_failure = "blank_current" if cur_blank.get("is_blank") else cur_error or "none"
    if not reference_img:
        ref_failure = "missing_reference"

    diff_failure = "none"
    diff_result = "not_compared"
    diff_img: Path | None = None
    mismatch_ratio: float | None = None
    normalized_masks: list[dict[str, Any]] = []
    diff_stats: dict[str, int | float] = {}
    if raw_masks and cur_w is not None and cur_h is not None:
        normalized_masks = normalize_masks(raw_masks, cur_w, cur_h)
    if ref_failure != "none":
        diff_failure = ref_failure
    elif cur_failure != "none":
        diff_failure = cur_failure
    elif ref_w != cur_w or ref_h != cur_h:
        diff_failure = "environment_mismatch"
    elif ref_pixels is not None and cur_pixels is not None and ref_w is not None and ref_h is not None:
        normalized_masks = normalize_masks(raw_masks, ref_w, ref_h)
        mismatch_ratio, diff_result, diff_pixels, diff_stats = compare_pixels(
            ref_pixels,
            cur_pixels,
            threshold,
            width=ref_w,
            height=ref_h,
            masks=normalized_masks,
        )
        diff_dir.mkdir(parents=True, exist_ok=True)
        diff_img = diff_dir / "diff.png"
        write_png_rgba(diff_img, ref_w, ref_h, diff_pixels)
        if diff_result == "fail":
            diff_failure = "diff_threshold_exceeded"
    else:
        diff_failure = "capture_failed"

    reference_report = report(
        root=root,
        artifact_kind="reference",
        image_path=reference_img,
        source_path=str(reference_src) if reference_src else None,
        source_command=args.reference_command,
        failure_reason=ref_failure,
        blank=ref_blank,
        width=ref_w,
        height=ref_h,
        reference_path=str(reference_img) if reference_img else None,
        current_path=str(current_img) if current_img else None,
        diff_path=str(diff_img) if diff_img else None,
        threshold=threshold,
        result="not_compared",
        masks=normalized_masks,
    )
    current_report = report(
        root=root,
        artifact_kind="current",
        image_path=current_img,
        source_path=str(current_src) if current_src else None,
        source_command=args.current_command,
        failure_reason=cur_failure,
        blank=cur_blank,
        width=cur_w,
        height=cur_h,
        reference_path=str(reference_img) if reference_img else None,
        current_path=str(current_img) if current_img else None,
        diff_path=str(diff_img) if diff_img else None,
        threshold=threshold,
        result="not_compared",
        masks=normalized_masks,
    )
    diff_report = report(
        root=root,
        artifact_kind="diff",
        image_path=diff_img,
        source_path=str(diff_img) if diff_img else None,
        source_command="visual-qa-artifacts.py compare",
        failure_reason=diff_failure,
        blank=blank_result(False, None),
        width=ref_w if ref_w == cur_w else None,
        height=ref_h if ref_h == cur_h else None,
        reference_path=str(reference_img) if reference_img else None,
        current_path=str(current_img) if current_img else None,
        diff_path=str(diff_img) if diff_img else None,
        threshold=threshold,
        result=diff_result,
        masks=normalized_masks,
    )
    if mismatch_ratio is not None:
        diff_report["comparison"]["mismatch_ratio"] = round(mismatch_ratio, 8)
        diff_report["comparison"].update(diff_stats)

    reference_dir.mkdir(parents=True, exist_ok=True)
    current_dir.mkdir(parents=True, exist_ok=True)
    diff_dir.mkdir(parents=True, exist_ok=True)
    (reference_dir / "report.json").write_text(json.dumps(reference_report, indent=2, ensure_ascii=False) + "\n")
    (current_dir / "report.json").write_text(json.dumps(current_report, indent=2, ensure_ascii=False) + "\n")
    (diff_dir / "report.json").write_text(json.dumps(diff_report, indent=2, ensure_ascii=False) + "\n")

    print(run_dir)
    if args.allow_missing_reference and diff_failure == "missing_reference" and cur_failure == "none":
        return 0
    return 0 if diff_failure == "none" else 1


def self_test() -> int:
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        ref = root / "ref.png"
        cur = root / "cur.png"
        blank = root / "blank.png"
        write_png_rgba(ref, 2, 2, [(0, 0, 0, 255), (255, 255, 255, 255), (10, 20, 30, 255), (40, 50, 60, 255)])
        write_png_rgba(cur, 2, 2, [(0, 0, 0, 255), (255, 255, 255, 255), (10, 20, 30, 255), (40, 50, 60, 255)])
        write_png_rgba(blank, 2, 2, [(255, 255, 255, 255)] * 4)
        masked_cur = root / "masked-cur.png"
        write_png_rgba(masked_cur, 2, 2, [(255, 0, 0, 255), (255, 255, 255, 255), (10, 20, 30, 255), (40, 50, 60, 255)])
        common = {
            "root": Path.cwd(),
            "out_dir": root / "out",
            "reference_command": "self",
            "current_command": "self",
            "allow_missing_reference": False,
            "mask_file": [],
        }
        ok = run(argparse.Namespace(**common, run_id="ok", reference=ref, current=cur, threshold=0.01, mask=[]))
        bad = run(argparse.Namespace(**common, run_id="blank", reference=blank, current=cur, threshold=0.01, mask=[]))
        masked = run(
            argparse.Namespace(
                **common,
                run_id="masked",
                reference=ref,
                current=masked_cur,
                threshold=0.0,
                mask=["0,0,1,1:live-content:expected fixture variance"],
            )
        )
        unmasked = run(argparse.Namespace(**common, run_id="unmasked", reference=ref, current=masked_cur, threshold=0.0, mask=[]))
        if ok != 0 or bad == 0 or masked != 0 or unmasked == 0:
            raise SystemExit("self-test failed")
    print("visual-qa-artifacts self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=Path(__file__).resolve().parents[1])
    parser.add_argument("--out-dir", default=".omx/artifacts/visual-qa")
    parser.add_argument("--run-id")
    parser.add_argument("--reference")
    parser.add_argument("--current", required=False)
    parser.add_argument("--threshold", default="0.01")
    parser.add_argument("--reference-command")
    parser.add_argument("--current-command")
    parser.add_argument("--mask", action="append", help="rect mask as x,y,width,height[:name[:reason]]; repeatable")
    parser.add_argument("--mask-file", action="append", help="JSON mask file containing a list or {masks:[...]}")
    parser.add_argument("--allow-missing-reference", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if not args.current:
        parser.error("--current is required unless --self-test is used")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
