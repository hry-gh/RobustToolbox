#!/usr/bin/env python3
# Packages the Robust.Client.WebView module for distribution via the launcher.
# Includes managed assemblies and CEF native binaries from the Robust.Natives.Cef NuGet package.

import os
import shutil
import subprocess
import sys
import zipfile
import argparse
import glob

from typing import List, Optional

try:
    from colorama import init, Fore, Style
    init()

except ImportError:
    # Just give an empty string for everything, no colored logging.
    class ColorDummy(object):
        def __getattr__(self, name):
            return ""

    Fore = ColorDummy()
    Style = ColorDummy()


p = os.path.join

PLATFORM_WINDOWS = "win-x64"
PLATFORM_LINUX = "linux-x64"
PLATFORM_MACOS_ARM64 = "osx-arm64"
PLATFORM_MACOS_X64 = "osx-x64"

ALL_PLATFORMS = [PLATFORM_WINDOWS, PLATFORM_LINUX, PLATFORM_MACOS_ARM64, PLATFORM_MACOS_X64]

TARGET_FRAMEWORK = "net10.0"

# Managed DLLs to include in every platform zip.
MANAGED_FILES = [
    "Robust.Client.WebView.dll",
    "Robust.Client.WebView.pdb",
    "Robust.Client.WebView.deps.json",
    # Dependencies needed when the module is loaded standalone.
    "Robust.Client.dll",
    "Robust.Shared.dll",
    "Robust.Shared.Maths.dll",
    "SharpZstd.Interop.dll",
]

# CEF native files per platform (allowlist).
# These are copied flat into the build output by the Robust.Natives.Cef MSBuild targets.
CEF_NATIVE_FILES_WINDOWS = [
    "robust_native_webview.dll",
    "Robust.Client.WebView.exe",
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

CEF_NATIVE_FILES_LINUX = [
    "librobust_native_webview.so",
    "Robust.Client.WebView",
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

CEF_NATIVE_FILES_MACOS = [
    "librobust_native_webview.dylib",
    "Robust.Client.WebView",
    "libEGL.dylib",
    "libGLESv2.dylib",
    "libvk_swiftshader.dylib",
    "vk_swiftshader_icd.json",
]


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Packages the Robust.Client.WebView module for release on all platforms.")
    parser.add_argument("--platform",
                        "-p",
                        action="store",
                        choices=ALL_PLATFORMS,
                        nargs="*",
                        help="Which platform to build for. If not provided, all platforms will be built")

    parser.add_argument("--skip-build",
                        action="store_true",
                        help=argparse.SUPPRESS)

    args = parser.parse_args()
    platforms = args.platform
    skip_build = args.skip_build

    if not platforms:
        platforms = ALL_PLATFORMS

    if os.path.exists("release"):
        print(Fore.BLUE + Style.DIM +
              "Cleaning old release packages (release/Robust.Client.WebView_*)..." + Style.RESET_ALL)
        for past in glob.glob("release/Robust.Client.WebView_*"):
            os.remove(past)
    else:
        os.mkdir("release")

    for platform in platforms:
        if not skip_build:
            wipe_bin()
        build_platform(platform, skip_build)


def wipe_bin():
    print(Fore.BLUE + Style.DIM +
          "Clearing old build artifacts (if any)..." + Style.RESET_ALL)

    RCWebViewBin = p("Robust.Client.WebView", "bin")
    if os.path.exists(RCWebViewBin):
        shutil.rmtree(RCWebViewBin)


def target_os_for_rid(rid: str) -> str:
    if rid.startswith("win"):
        return "Windows"
    elif rid.startswith("linux"):
        return "Linux"
    elif rid.startswith("osx"):
        return "MacOS"
    raise ValueError(f"Unknown RID: {rid}")


def build_platform(rid: str, skip_build: bool) -> None:
    print(Fore.GREEN + f"Building and packaging {rid}..." + Style.RESET_ALL)

    target_os = target_os_for_rid(rid)
    base_bin = p("Robust.Client.WebView", "bin", "Release", TARGET_FRAMEWORK)

    if not skip_build:
        # RID-specific build to pull in native files from NuGet targets.
        subprocess.run([
            "dotnet", "build",
            "-c", "Release",
            "-r", rid,
            f"/p:TargetOS={target_os}",
            "/p:FullRelease=True",
            "--no-self-contained",
            "Robust.Client.WebView/Robust.Client.WebView.csproj",
        ], check=True)

    rid_bin = p(base_bin, rid)

    print(Fore.GREEN + f"Packaging {rid}..." + Style.RESET_ALL)

    client_zip = zipfile.ZipFile(
        p("release", f"Robust.Client.WebView_{rid}.zip"), "w",
        compression=zipfile.ZIP_DEFLATED)

    # Copy managed assemblies.
    for f in MANAGED_FILES:
        src = p(rid_bin, f)
        if os.path.exists(src):
            client_zip.write(src, f)
        else:
            print(Fore.YELLOW + f"  WARNING: managed file {f} not found" + Style.RESET_ALL)

    # Copy CEF native files.
    if rid.startswith("win"):
        native_files = CEF_NATIVE_FILES_WINDOWS
    elif rid.startswith("linux"):
        native_files = CEF_NATIVE_FILES_LINUX
    elif rid.startswith("osx"):
        native_files = CEF_NATIVE_FILES_MACOS
    else:
        native_files = []

    for f in native_files:
        src = p(rid_bin, f)
        if os.path.exists(src):
            client_zip.write(src, f)
        else:
            print(Fore.YELLOW + f"  WARNING: native file {f} not found" + Style.RESET_ALL)

    # Copy locales directory (Windows/Linux).
    if rid.startswith("win") or rid.startswith("linux"):
        locales_dir = p(rid_bin, "locales")
        if os.path.isdir(locales_dir):
            copy_dir_into_zip(locales_dir, "locales", client_zip)

    # Copy macOS framework bundle.
    if rid.startswith("osx"):
        fw_dir = p(rid_bin, "Frameworks", "Chromium Embedded Framework.framework")
        if os.path.isdir(fw_dir):
            copy_dir_into_zip(fw_dir, p("Frameworks", "Chromium Embedded Framework.framework"), client_zip)
        else:
            print(Fore.RED + f"  ERROR: Framework not found at {fw_dir}" + Style.RESET_ALL)

    client_zip.close()
    print(Fore.GREEN + f"  Created release/Robust.Client.WebView_{rid}.zip" + Style.RESET_ALL)


def zip_entry_exists(zipf, name):
    try:
        # Trick ZipInfo into sanitizing the name for us so this awful module stops spewing warnings.
        zinfo = zipfile.ZipInfo.from_file("Resources", name)
        zipf.getinfo(zinfo.filename)
    except KeyError:
        return False
    return True


def copy_dir_into_zip(directory, basepath, zipf, ignored={}):
    if basepath and not zip_entry_exists(zipf, basepath):
        zipf.write(directory, basepath)

    for root, _, files in os.walk(directory):
        relpath = os.path.relpath(root, directory)
        if relpath != "." and not zip_entry_exists(zipf, p(basepath, relpath)):
            zipf.write(root, p(basepath, relpath))

        for filename in files:
            zippath = p(basepath, relpath, filename)
            filepath = p(root, filename)

            if filename in ignored:
                continue

            message = "{dim}{diskroot}{sep}{zipfile}{dim} -> {ziproot}{sep}{zipfile}".format(
                sep=os.sep + Style.NORMAL,
                dim=Style.DIM,
                diskroot=directory,
                ziproot=zipf.filename,
                zipfile=os.path.normpath(zippath))

            print(Fore.CYAN + message + Style.RESET_ALL)
            zipf.write(filepath, zippath)


if __name__ == '__main__':
    main()
