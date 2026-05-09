#!/usr/bin/env python3
"""Packaged macOS window chrome smoke test for ModelRack.

Launches build/ModelRack.app with isolated prefs, drives the custom Slint
traffic-light controls with CoreGraphics mouse events, and records objective
window-state evidence for TODO-15.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "build" / "ModelRack.app"
PROCESS = "modelrack"
DEFAULT_TIMEOUT = 8


@dataclass
class Bounds:
    x: int
    y: int
    width: int
    height: int

    @property
    def area(self) -> int:
        return self.width * self.height


@dataclass
class Check:
    name: str
    passed: bool
    evidence: dict


def run(cmd: list[str], *, check: bool = True, env: dict[str, str] | None = None, timeout: int = DEFAULT_TIMEOUT) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=check,
        env=env,
        timeout=timeout,
    )


def osascript(script: str, *, timeout: int = DEFAULT_TIMEOUT) -> str:
    result = run(["osascript", "-e", script], timeout=timeout)
    return result.stdout.strip()


def get_bounds() -> Bounds:
    raw = osascript(f'tell application "System Events" to tell process "{PROCESS}" to get position of window 1 & size of window 1')
    parts = [int(part.strip()) for part in raw.split(",")]
    if len(parts) != 4:
        raise RuntimeError(f"Unexpected bounds response: {raw!r}")
    return Bounds(*parts)


def window_count() -> int:
    raw = osascript(f'tell application "System Events" to tell process "{PROCESS}" to count windows')
    return int(raw)


def process_visible() -> bool:
    raw = osascript(f'tell application "System Events" to tell process "{PROCESS}" to get visible')
    return raw.lower() == "true"


def ax_bool(attribute: str) -> bool:
    raw = osascript(
        f'tell application "System Events" to tell process "{PROCESS}" to get value of attribute "{attribute}" of window 1'
    )
    return raw.lower() == "true"


def native_window_button_count() -> int:
    raw = osascript(f'tell application "System Events" to tell process "{PROCESS}" to count buttons of window 1')
    return int(raw)


def parse_applescript_list(raw: str) -> list[str]:
    return [part.strip() for part in raw.split("||") if part.strip()]


def menu_bar_snapshot() -> dict[str, list[str]]:
    script = f'''
tell application "System Events"
    tell process "{PROCESS}"
        set previousDelimiters to AppleScript's text item delimiters
        set AppleScript's text item delimiters to "||"
        set topItems to name of menu bar items of menu bar 1
        set topLine to topItems as text
        set appItems to {{}}
        repeat with candidate in menu bar items of menu bar 1
            set candidateName to name of candidate
            if candidateName is "ModelRack" or candidateName is "{PROCESS}" then
                set appItems to name of menu items of menu 1 of candidate
            end if
        end repeat
        set appLine to appItems as text
        set AppleScript's text item delimiters to previousDelimiters
        return topLine & linefeed & appLine
    end tell
end tell
'''
    raw = osascript(script)
    top_line, _, app_line = raw.partition("\n")
    return {
        "top_level": parse_applescript_list(top_line),
        "app_menu": parse_applescript_list(app_line),
    }


def wait_for_bounds(timeout: float = 6.0) -> Bounds:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            return get_bounds()
        except Exception as exc:  # noqa: BLE001 - diagnostics written to report on failure
            last_error = exc
            time.sleep(0.2)
    raise RuntimeError(f"Window bounds unavailable: {last_error}")


def wait_until(predicate, timeout: float = 6.0, interval: float = 0.2) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            if predicate():
                return True
        except Exception:
            pass
        time.sleep(interval)
    return False


def install_click_helper(out_dir: Path) -> Path:
    helper = out_dir / "cg_event.swift"
    helper.write_text(
        """
import Foundation
import CoreGraphics

func post(_ event: CGEvent?) {
    event?.post(tap: .cghidEventTap)
    usleep(50_000)
}

let mode = CommandLine.arguments[1]
let x = Double(CommandLine.arguments[2])!
let y = Double(CommandLine.arguments[3])!
let source = CGEventSource(stateID: .hidSystemState)

if mode == "click" {
    let point = CGPoint(x: x, y: y)
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: point, mouseButton: .left))
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: point, mouseButton: .left))
} else if mode == "move" {
    let point = CGPoint(x: x, y: y)
    post(CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left))
} else if mode == "drag" {
    let dx = Double(CommandLine.arguments[4])!
    let dy = Double(CommandLine.arguments[5])!
    let start = CGPoint(x: x, y: y)
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: start, mouseButton: .left))
    for step in 1...8 {
        let t = Double(step) / 8.0
        let point = CGPoint(x: x + dx * t, y: y + dy * t)
        post(CGEvent(mouseEventSource: source, mouseType: .leftMouseDragged, mouseCursorPosition: point, mouseButton: .left))
    }
    let end = CGPoint(x: x + dx, y: y + dy)
    post(CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: end, mouseButton: .left))
} else {
    fputs("unknown mode\\n", stderr)
    exit(2)
}
""".strip()
        + "\n"
    )
    return helper


def click(helper: Path, x: int, y: int) -> None:
    run(["swift", str(helper), "click", str(x), str(y)], timeout=10)


def move_mouse(helper: Path, x: int, y: int) -> None:
    run(["swift", str(helper), "move", str(x), str(y)], timeout=10)


def drag(helper: Path, x: int, y: int, dx: int, dy: int) -> None:
    run(["swift", str(helper), "drag", str(x), str(y), str(dx), str(dy)], timeout=10)


def click_native_window_button(description: str) -> None:
    osascript(
        f'tell application "System Events" to tell process "{PROCESS}" to click '
        f'(first button of window 1 whose description is "{description}")',
        timeout=5,
    )


def toggle_fullscreen_shortcut() -> None:
    osascript('tell application "System Events" to key code 3 using {control down, command down}', timeout=5)


def rel(bounds: Bounds, dx: int, dy: int) -> tuple[int, int]:
    return bounds.x + dx, bounds.y + dy


def close_center(bounds: Bounds) -> tuple[int, int]:
    return rel(bounds, 18, 18)


def minimize_center(bounds: Bounds) -> tuple[int, int]:
    return rel(bounds, 38, 18)


def green_center(bounds: Bounds) -> tuple[int, int]:
    return rel(bounds, 58, 18)


def titlebar_drag_start(bounds: Bounds) -> tuple[int, int]:
    return rel(bounds, 260, 18)


def restore_from_hidden() -> None:
    run(["open", "-a", str(APP)], check=False)
    osascript(f'tell application "{PROCESS}" to activate', timeout=4)


def restore_minimized() -> None:
    osascript(f'tell application "System Events" to tell process "{PROCESS}" to set value of attribute "AXMinimized" of window 1 to false')
    osascript(f'tell application "{PROCESS}" to activate', timeout=4)


def launch_app(out_dir: Path) -> None:
    if not APP.is_dir():
        raise RuntimeError(f"Missing app bundle: {APP}")
    prefs = out_dir / "prefs.json"
    prefs.write_text(json.dumps({"density": "medium", "view_mode": "grid"}) + "\n")
    run(["pkill", "-x", PROCESS], check=False)
    time.sleep(0.4)
    env = os.environ.copy()
    env["MODELRACK_PREFS_PATH"] = str(prefs)
    run(["open", "-n", str(APP), "--env", f"MODELRACK_PREFS_PATH={prefs}"], env=env)
    wait_for_bounds()
    osascript(f'tell application "{PROCESS}" to activate', timeout=4)
    time.sleep(0.6)


def run_capture_smoke(out_dir: Path) -> list[str]:
    result = run([str(ROOT / "scripts" / "capture-smoke.sh"), str(out_dir / "capture-smoke")], timeout=30)
    return [line for line in result.stdout.splitlines() if line.strip()]


def rgb_distance(left: tuple[int, int, int], right: tuple[int, int, int]) -> int:
    return sum(abs(a - b) for a, b in zip(left, right))


def verify_rounded_window_corners(capture_paths: list[str]) -> Check:
    if not capture_paths:
        return Check("native_rounded_window_corners_visible", False, {"error": "no screenshot path emitted"})

    screenshot = Path(capture_paths[0])
    if not screenshot.exists():
        return Check(
            "native_rounded_window_corners_visible",
            False,
            {"error": "screenshot path does not exist", "path": str(screenshot)},
        )

    try:
        from PIL import Image
    except ImportError as exc:
        return Check(
            "native_rounded_window_corners_visible",
            False,
            {"error": f"Pillow is required for rounded-corner image QA: {exc}"},
        )

    with Image.open(screenshot) as image:
        pixels = image.convert("RGB")
        width, height = pixels.size

        # capture-smoke crops by logical AX bounds; screenshots on Retina are
        # normally 2x. Keep this derived from the actual image so non-Retina
        # CI/manual hosts still work.
        scale = max(1, round(width / 1480))
        radius = 12 * scale
        inset = max(3, radius // 5)
        interior_offset = max(radius * 2, 32 * scale)

        samples = {
            "top_left": ((inset, inset), (interior_offset, interior_offset)),
            "top_right": ((width - inset - 1, inset), (width - interior_offset - 1, interior_offset)),
            "bottom_left": ((inset, height - inset - 1), (interior_offset, height - interior_offset - 1)),
            "bottom_right": (
                (width - inset - 1, height - inset - 1),
                (width - interior_offset - 1, height - interior_offset - 1),
            ),
        }

        evidence = {}
        visible_corners = 0
        for name, (corner_point, interior_point) in samples.items():
            corner_rgb = pixels.getpixel(corner_point)
            interior_rgb = pixels.getpixel(interior_point)
            distance = rgb_distance(corner_rgb, interior_rgb)
            evidence[name] = {
                "corner_point": corner_point,
                "corner_rgb": corner_rgb,
                "interior_point": interior_point,
                "interior_rgb": interior_rgb,
                "rgb_distance": distance,
            }
            if distance >= 24:
                visible_corners += 1

    evidence["image"] = {"path": str(screenshot), "width": width, "height": height, "scale": scale}
    evidence["threshold"] = {"minimum_visible_corners": 3, "minimum_rgb_distance": 24}
    return Check("native_rounded_window_corners_visible", visible_corners >= 3, evidence)


def main() -> int:
    run_id = "window-chrome-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out_dir = ROOT / ".omx" / "artifacts" / "window-chrome-qa" / run_id
    out_dir.mkdir(parents=True, exist_ok=True)
    helper = install_click_helper(out_dir)
    checks: list[Check] = []

    launch_app(out_dir)
    initial = wait_for_bounds()
    checks.append(
        Check(
            "initial_packaged_geometry",
            initial.width >= 960 and initial.height >= 640,
            {"bounds": asdict(initial), "expected_requested": {"width": 1480, "height": 920}},
        )
    )

    menus = menu_bar_snapshot()
    top_level = set(menus["top_level"])
    app_menu = set(menus["app_menu"])
    checks.append(
        Check(
            "native_macos_menu_bar_contains_app_settings",
            {"File", "Edit", "View", "Window", "Help"}.issubset(top_level)
            and ("ModelRack" in top_level or PROCESS in top_level)
            and "Settings…" in app_menu,
            menus,
        )
    )
    native_buttons = native_window_button_count()
    checks.append(
        Check(
            "native_standard_traffic_lights_hidden_no_duplicate",
            native_buttons == 0,
            {"accessibility_button_count": native_buttons},
        )
    )

    # Drag the custom titlebar and verify only position changes.
    sx, sy = titlebar_drag_start(initial)
    drag(helper, sx, sy, 42, 28)
    time.sleep(0.8)
    dragged = get_bounds()
    checks.append(
        Check(
            "custom_titlebar_drag_moves_window",
            (dragged.x, dragged.y) != (initial.x, initial.y)
            and abs(dragged.width - initial.width) <= 2
            and abs(dragged.height - initial.height) <= 2,
            {"before": asdict(initial), "after": asdict(dragged)},
        )
    )

    # Red uses the custom Slint traffic light while the NSWindow wrapper stays native.
    cx, cy = close_center(dragged)
    click(helper, cx, cy)
    hidden = wait_until(lambda: not process_visible(), timeout=5)
    restore_from_hidden()
    restored_after_hide = wait_for_bounds()
    visible_after_restore = process_visible()
    checks.append(
        Check(
            "red_custom_light_hides_and_activation_restores",
            hidden and visible_after_restore and restored_after_hide.width > 0,
            {
                "hidden_observed": hidden,
                "visible_after_restore": visible_after_restore,
                "bounds_after_restore": asdict(restored_after_hide),
            },
        )
    )

    # Yellow uses the custom Slint traffic light; explicit deminiaturize restore returns the window.
    mx, my = minimize_center(restored_after_hide)
    click(helper, mx, my)
    minimized = wait_until(lambda: ax_bool("AXMinimized"), timeout=5)
    restore_minimized()
    restored_after_minimize = wait_for_bounds()
    minimized_after_restore = ax_bool("AXMinimized")
    checks.append(
        Check(
            "yellow_minimizes_and_restores",
            minimized and not minimized_after_restore and restored_after_minimize.width > 0,
            {
                "minimized_observed": minimized,
                "minimized_after_restore": minimized_after_restore,
                "bounds_after_restore": asdict(restored_after_minimize),
            },
        )
    )

    # Green enters native macOS full-screen and a second press restores previous geometry.
    before_green = get_bounds()
    gx, gy = green_center(before_green)
    click(helper, gx, gy)
    fullscreen_attr = wait_until(lambda: ax_bool("AXFullScreen"), timeout=7)
    fullscreen_bounds = get_bounds()
    osascript(f'tell application "{PROCESS}" to activate', timeout=4)
    move_mouse(helper, fullscreen_bounds.x + 10, fullscreen_bounds.y + 1)
    time.sleep(3.5)
    gx2, gy2 = green_center(fullscreen_bounds)
    click(helper, gx2, gy2)
    exited_fullscreen = wait_until(lambda: not ax_bool("AXFullScreen"), timeout=5)
    if not exited_fullscreen:
        click(helper, gx2, gy2)
        exited_fullscreen = wait_until(lambda: not ax_bool("AXFullScreen"), timeout=8)
    restored_after_green = get_bounds()
    size_restored = False
    if exited_fullscreen:
        wait_until(
            lambda: (
                abs((latest := get_bounds()).width - before_green.width) <= 4
                and abs(latest.height - before_green.height) <= 4
            ),
            timeout=6,
        )
        restored_after_green = get_bounds()
        size_restored = (
            abs(restored_after_green.width - before_green.width) <= 4
            and abs(restored_after_green.height - before_green.height) <= 4
        )
    checks.append(
        Check(
            "green_enters_native_fullscreen_and_restores",
            fullscreen_attr and exited_fullscreen and size_restored,
            {
                "before": asdict(before_green),
                "fullscreen": asdict(fullscreen_bounds),
                "restored": asdict(restored_after_green),
                "ax_fullscreen_after_green": fullscreen_attr,
                "ax_fullscreen_after_restore": not exited_fullscreen,
            },
        )
    )

    capture_paths = run_capture_smoke(out_dir)
    checks.append(
        Check(
            "capture_smoke_emits_artifacts",
            len(capture_paths) >= 2 and all(Path(path).exists() for path in capture_paths[:2]),
            {"paths": capture_paths},
        )
    )
    checks.append(verify_rounded_window_corners(capture_paths))

    passed = all(check.passed for check in checks)
    report = {
        "run_id": run_id,
        "app": str(APP),
        "result": "passed" if passed else "failed",
        "checks": [asdict(check) for check in checks],
    }
    report_path = out_dir / "window-chrome-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print(report_path)
    print(json.dumps({"result": report["result"], "checks": {check.name: check.passed for check in checks}}, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        sys.stderr.write(exc.stdout)
        sys.stderr.write(exc.stderr)
        raise
