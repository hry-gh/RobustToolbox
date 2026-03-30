#!/usr/bin/env python3
"""
Build and pack the Robust.Natives.Cef NuGet package.

Prerequisites:
  - Rust toolchain with targets: x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu,
    aarch64-apple-darwin, x86_64-apple-darwin
  - CEF binaries downloaded (or let cef-dll-sys download them during cargo build)
  - nuget or dotnet pack available

Usage:
  python3 pack.py [--cef-version 146.0.6] [--skip-build]

The script:
  1. Builds robust-native-webview and cef-helper for each target
  2. Collects CEF runtime files from the cargo build output
  3. Assembles the runtimes/ directory structure
  4. Packs the NuGet package
"""

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
NATIVE_DIR = SCRIPT_DIR.parent
NUGET_DIR = SCRIPT_DIR

TARGETS = {
    "win-x64": {
        "triple": "x86_64-pc-windows-msvc",
        "cef_platform": "cef_windows_x86_64",
        "webview_lib": "robust_native_webview.dll",
        "helper_bin": "cef-helper.exe",
    },
    "linux-x64": {
        "triple": "x86_64-unknown-linux-gnu",
        "cef_platform": "cef_linux_x86_64",
        "webview_lib": "librobust_native_webview.so",
        "helper_bin": "cef-helper",
    },
    "osx-arm64": {
        "triple": "aarch64-apple-darwin",
        "cef_platform": "cef_macos_aarch64",
        "webview_lib": "librobust_native_webview.dylib",
        "helper_bin": "cef-helper",
    },
    "osx-x64": {
        "triple": "x86_64-apple-darwin",
        "cef_platform": "cef_macos_x86_64",
        "webview_lib": "librobust_native_webview.dylib",
        "helper_bin": "cef-helper",
    },
}

# CEF files to include per platform (relative to CEF dir)
CEF_FILES_WINDOWS = [
    "libcef.dll",
    "chrome_elf.dll",
    "libEGL.dll",
    "libGLESv2.dll",
    "v8_context_snapshot.bin",
    "vk_swiftshader.dll",
    "vk_swiftshader_icd.json",
    "vulkan-1.dll",
    "icudtl.dat",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "resources.pak",
]

CEF_FILES_LINUX = [
    "libcef.so",
    "libEGL.so",
    "libGLESv2.so",
    "libvk_swiftshader.so",
    "vk_swiftshader_icd.json",
    "v8_context_snapshot.bin",
    "icudtl.dat",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "resources.pak",
]


def _copy_en_locale(cef_dir: Path, out_dir: Path):
    locale_src = cef_dir / "locales" / "en-US.pak"
    if locale_src.exists():
        locale_dst = out_dir / "locales"
        locale_dst.mkdir(exist_ok=True)
        shutil.copy2(locale_src, locale_dst / "en-US.pak")
        print("  Copied locales/en-US.pak")


def find_cef_dir(target_dir: Path, cef_platform: str) -> Path | None:
    """Find the CEF directory in cargo's build output."""
    build_dir = target_dir / "build"
    if not build_dir.exists():
        return None
    for d in build_dir.iterdir():
        if d.name.startswith("cef-dll-sys-"):
            cef_path = d / "out" / cef_platform
            if cef_path.exists():
                return cef_path
    return None


def build_target(triple: str, skip_build: bool):
    """Build robust-native-webview and cef-helper for a target."""
    if skip_build:
        print(f"  Skipping build for {triple}")
        return

    print(f"  Building for {triple}...")
    subprocess.run(
        [
            "cargo", "build",
            "--release",
            "--target", triple,
            "-p", "robust-native-webview",
            "-p", "cef-helper",
        ],
        cwd=NATIVE_DIR,
        check=True,
    )


def collect_runtime(rid: str, info: dict, skip_build: bool):
    """Collect runtime files for a platform into the runtimes/ directory."""
    triple = info["triple"]
    target_dir = NATIVE_DIR / "target" / triple / "release"
    out_dir = NUGET_DIR / "runtimes" / rid / "native"

    print(f"Collecting {rid}...")

    build_target(triple, skip_build)

    # Clean and create output directory
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)

    # Copy webview library
    webview_src = target_dir / info["webview_lib"]
    if webview_src.exists():
        shutil.copy2(webview_src, out_dir / info["webview_lib"])
        print(f"  Copied {info['webview_lib']}")
    else:
        print(f"  WARNING: {webview_src} not found!")

    # Copy cef-helper
    helper_src = target_dir / info["helper_bin"]
    if helper_src.exists():
        shutil.copy2(helper_src, out_dir / info["helper_bin"])
        print(f"  Copied {info['helper_bin']}")
    else:
        print(f"  WARNING: {helper_src} not found!")

    # Find and copy CEF files
    cef_dir = find_cef_dir(NATIVE_DIR / "target" / triple / "release", info["cef_platform"])
    if cef_dir is None:
        # Try the default target dir (no cross-compilation)
        cef_dir = find_cef_dir(NATIVE_DIR / "target" / "release", info["cef_platform"])

    if cef_dir is None:
        print(f"  WARNING: CEF directory not found for {rid}!")
        return

    if rid.startswith("osx"):
        fw_src = cef_dir / "Chromium Embedded Framework.framework"
        fw_dst = out_dir / "Chromium Embedded Framework.framework"
        if fw_src.exists():
            shutil.copytree(fw_src, fw_dst, symlinks=True,
                            ignore=shutil.ignore_patterns("*.lproj"))
            en_lproj = fw_src / "Resources" / "en.lproj"
            if en_lproj.exists():
                shutil.copytree(en_lproj, fw_dst / "Resources" / "en.lproj", symlinks=True)
            print("  Copied Chromium Embedded Framework.framework (en locale only)")

            # Copy ANGLE/Vulkan libs flat alongside the executable so CEF subprocesses can find them
            for lib in ["libEGL.dylib", "libGLESv2.dylib", "libvk_swiftshader.dylib", "vk_swiftshader_icd.json"]:
                lib_src = fw_src / "Libraries" / lib
                if lib_src.exists():
                    shutil.copy2(lib_src, out_dir / lib)
                    print(f"  Copied flat {lib}")
        else:
            print(f"  WARNING: Framework not found at {fw_src}")
    elif rid.startswith("win"):
        for f in CEF_FILES_WINDOWS:
            src = cef_dir / f
            if src.exists():
                shutil.copy2(src, out_dir / f)
            else:
                print(f"  WARNING: {f} not found in CEF dir")
        _copy_en_locale(cef_dir, out_dir)
    elif rid.startswith("linux"):
        for f in CEF_FILES_LINUX:
            src = cef_dir / f
            if src.exists():
                shutil.copy2(src, out_dir / f)
            else:
                print(f"  WARNING: {f} not found in CEF dir")
        _copy_en_locale(cef_dir, out_dir)


def main():
    parser = argparse.ArgumentParser(description="Pack Robust.Natives.Cef NuGet package")
    parser.add_argument("--skip-build", action="store_true", help="Skip cargo build, use existing artifacts")
    parser.add_argument("--targets", nargs="*", choices=list(TARGETS.keys()), default=list(TARGETS.keys()),
                        help="Which targets to build (default: all)")
    args = parser.parse_args()

    print("Collecting runtime files...")
    for rid in args.targets:
        collect_runtime(rid, TARGETS[rid], args.skip_build)

    print("\nPacking NuGet package...")
    subprocess.run(
        ["nuget", "pack", str(NUGET_DIR / "Robust.Natives.Cef.nuspec"), "-OutputDirectory", str(NUGET_DIR)],
        check=True,
    )
    print("Done!")


if __name__ == "__main__":
    main()
