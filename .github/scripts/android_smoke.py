#!/usr/bin/env python3
"""Install the real Codex APK on an ARM64 Android device and verify the app-server handshake.

This test is intentionally strict: it does not accept a screenshot-only placeholder,
a package without the Rust core, a launcher that crashes, or an unconnected UI.
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import time
import zipfile
from pathlib import Path

PACKAGE = "com.codex.mobile"
ACTIVITY = PACKAGE + "/.MainActivity"
CORE_TEMPLATE = "lib/{abi}/libcodex.so"


def run(*args: str, timeout: int = 35, check: bool = True) -> str:
    command = ["adb", *args]
    result = subprocess.run(command, capture_output=True, text=True, timeout=timeout)
    if check and result.returncode != 0:
        raise RuntimeError(
            f"{' '.join(command)} exited {result.returncode}\n"
            + result.stdout[-1000:]
            + result.stderr[-1000:]
        )
    return result.stdout + result.stderr


def dump_ui() -> str:
    run("shell", "uiautomator", "dump", "/sdcard/codex-ui.xml", timeout=40)
    return run("shell", "cat", "/sdcard/codex-ui.xml", timeout=20)


def capture_failure() -> None:
    print("Recent app logs:", file=sys.stderr)
    logcat = run("logcat", "-d", "-t", "1000", check=False)
    lines = [
        x for x in logcat.splitlines()
        if any(k in x for k in ["CodexRuntime", "Codex", "AndroidRuntime", "FATAL EXCEPTION"])
    ]
    print("\n".join(lines[-70:]), file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apk", type=Path, required=True)
    parser.add_argument("--abi", default="arm64-v8a", choices=("arm64-v8a", "x86_64"))
    parser.add_argument("--min-sdk", type=int, default=37,
                        help="Android 17 / API 37 by default")
    parser.add_argument("--timeout", type=int, default=75)
    args = parser.parse_args()

    if not args.apk.is_file():
        raise FileNotFoundError(args.apk)
    core = CORE_TEMPLATE.format(abi=args.abi)
    with zipfile.ZipFile(args.apk) as apk:
        if core not in apk.namelist():
            raise RuntimeError(f"APK 缺少原生 Codex：{core}")
        elf = apk.read(core)
        if elf[:4] != b"\x7fELF" or elf[4] != 2:
            raise RuntimeError("原生核心不是有效的 64 位 ELF")
        print(f"Verified embedded native Rust core: {len(elf):,} bytes")

    run("wait-for-device", timeout=90)
    sdk = int(run("shell", "getprop", "ro.build.version.sdk").strip())
    if sdk < args.min_sdk:
        raise RuntimeError(f"设备 Android API {sdk} 低于要求 {args.min_sdk}")
    abi = run("shell", "getprop", "ro.product.cpu.abilist")
    if args.abi not in abi:
        raise RuntimeError(f"设备缺少 {args.abi} ABI: {abi}")
    print(f"Detected Android API {sdk}, ABI {args.abi}")

    run("install", "-r", str(args.apk.absolute()), timeout=150)
    run("shell", "am", "force-stop", PACKAGE)
    run("shell", "wm", "dismiss-keyguard", check=False)
    start = run("shell", "am", "start", "-n", ACTIVITY)
    if "Error" in start:
        raise RuntimeError("启动主界面失败：" + start)

    deadline = time.monotonic() + args.timeout
    previous = ""
    while time.monotonic() < deadline:
        try:
            if not run("shell", "pidof", PACKAGE, check=False).strip():
                raise RuntimeError("Codex Android 应用进程已退出")
            xml = dump_ui()
            previous = xml
            if "已连接 · 原生 Codex" in xml or "已连接 &#183; 原生 Codex" in xml:
                print("PASS: Android UI launched, native app-server initialized.")
                return 0
            if "原生核心启动失败" in xml or "操作未完成" in xml:
                raise RuntimeError("界面报告原生核心启动错误")
        except RuntimeError as error:
            if "应用进程已退出" in str(error) or "界面报告原生核心启动错误" in str(error):
                raise
            # UIAutomator can fail before the first frame is drawn; retry the
            # screenshot/hierarchy read until the global deadline.
            previous = str(error)
        except (subprocess.TimeoutExpired, OSError) as error:
            previous = str(error)
        time.sleep(2)

    print("Last UI dump:", previous[-4000:], file=sys.stderr)
    raise RuntimeError("未检测到已连接的 Codex App Server")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print("FAIL:", error, file=sys.stderr)
        try:
            capture_failure()
        except Exception:
            pass
        sys.exit(1)
