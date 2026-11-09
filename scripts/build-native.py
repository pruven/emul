#!/usr/bin/env python3
"""Create macOS app/DMG/tar and Windows EXE/ZIP/tar bundles on native runners.

Windows builds FFmpeg in the workflow's MSYS2 shell between --prepare-media and
--finish-media; all other packaging uses the host Python and MSVC Rust toolchain.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import tarfile
import zipfile

from bundle_common import ROOT, check_architecture, checksum, download, host_arch, run, tar_bundle, version

SYSTEM = {"Darwin": "macos", "Windows": "windows"}.get(platform.system())
ARCH = host_arch()
TARGET = f"{SYSTEM}-{ARCH}"
BUILD = ROOT / "target/bundles" / TARGET
CACHE = BUILD / "downloads"
PREFIX = BUILD / "media"
SOURCE = BUILD / "media-source"
RECIPE = ROOT / "packaging/build-media.sh"
SUFFIX = ".exe" if SYSTEM == "windows" else ""


def specifications():
    common = json.loads((ROOT / "packaging/linux/downloads.json").read_text())
    specs = {name: common[name] for name in (
        "ffmpeg-source.tar.gz", "lame-source.tar.gz", "yt-dlp-source.tar.gz", "deno-LICENSE.md")}
    specs.update(json.loads((ROOT / "packaging/native-downloads.json").read_text())[TARGET])
    return specs


def media_stamp(specs):
    sources = {name: specs[name] for name in ("ffmpeg-source.tar.gz", "lame-source.tar.gz")}
    return hashlib.sha256(RECIPE.read_bytes() + TARGET.encode() + json.dumps(sources, sort_keys=True).encode()).hexdigest()


def media_ready(specs):
    stamp = PREFIX / ".stamp"
    return stamp.exists() and stamp.read_text() == media_stamp(specs)


def prepare_media(specs):
    if media_ready(specs):
        return
    for directory in (SOURCE, PREFIX):
        if directory.exists():
            shutil.rmtree(directory)
    SOURCE.mkdir(parents=True)
    for name in ("lame", "ffmpeg"):
        with tarfile.open(CACHE / f"{name}-source.tar.gz") as archive:
            archive.extractall(SOURCE, filter="data")


def finish_media(specs):
    for name in ("ffmpeg", "ffprobe"):
        executable = PREFIX / "bin" / f"{name}{SUFFIX}"
        check_architecture(executable, SYSTEM, ARCH)
        # No compiler/MSYS2 directories on PATH: missing runtime DLLs must fail here.
        system_path = str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32") if SYSTEM == "windows" else "/usr/bin:/bin"
        run(executable, "-version", env={**os.environ, "PATH": system_path})
    (PREFIX / ".stamp").write_text(media_stamp(specs))


def notices(destination, specs):
    destination.mkdir(parents=True)
    shutil.copy2(ROOT / "LICENSE", destination)
    shutil.copy2(ROOT / "packaging/THIRD-PARTY.md", destination)
    shutil.copy2(CACHE / "deno-LICENSE.md", destination)
    (destination / "downloads.json").write_text(json.dumps(specs, indent=2) + "\n")
    for directory in ("FFmpeg-n8.0.1", "lame-3.100"):
        for license_file in (SOURCE / directory).glob("COPYING*"):
            shutil.copy2(license_file, destination / f"{directory}-{license_file.name}")
    with tarfile.open(CACHE / "yt-dlp-source.tar.gz") as archive:
        member = archive.getmember("yt-dlp/LICENSE")
        (destination / "yt-dlp-LICENSE").write_bytes(archive.extractfile(member).read())


def package(specs):
    release = version()
    run("cargo", "build", "--release", "--locked", cwd=ROOT)
    stage = BUILD / f"emul-{release}-{TARGET}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir()
    if SYSTEM == "macos":
        app = stage / "emul.app"
        executable_dir = app / "Contents/MacOS"
        resources = app / "Contents/Resources"
        resources.mkdir(parents=True)
        info = {"CFBundleName": "emul", "CFBundleDisplayName": "emul",
                "CFBundleIdentifier": "org.emul.downloader", "CFBundleExecutable": "emul",
                "CFBundlePackageType": "APPL", "CFBundleShortVersionString": release,
                "CFBundleVersion": release, "LSMinimumSystemVersion": "14.0",
                "NSHighResolutionCapable": True}
        (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
        docs = resources / "licenses"
    else:
        executable_dir = stage
        docs = stage / "licenses"
    tools = executable_dir / "tools"
    tools.mkdir(parents=True)
    binary = executable_dir / f"emul{SUFFIX}"
    shutil.copy2(ROOT / "target/release" / binary.name, binary)
    binary.chmod(0o755)
    for name in ("ffmpeg", "ffprobe"):
        shutil.copy2(PREFIX / "bin" / f"{name}{SUFFIX}", tools)
    shutil.copy2(CACHE / "yt-dlp", tools / f"yt-dlp{SUFFIX}")
    with zipfile.ZipFile(CACHE / "deno.zip") as archive:
        (tools / f"deno{SUFFIX}").write_bytes(archive.read(f"deno{SUFFIX}"))
    for tool in tools.iterdir():
        tool.chmod(0o755)
        check_architecture(tool, SYSTEM, ARCH)
    check_architecture(binary, SYSTEM, ARCH)
    notices(docs, specs)
    if SYSTEM == "macos":
        # Preserve the upstream signatures and appended data of yt-dlp and Deno.
        for name in ("ffmpeg", "ffprobe"):
            run("codesign", "--force", "--sign", "-", tools / name)
        run("codesign", "--force", "--sign", "-", app)
        run("codesign", "--verify", "--strict", app)
    system_path = str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32") if SYSTEM == "windows" else "/usr/bin:/bin"
    run(binary, "--check-tools", env={**os.environ, "PATH": system_path})
    (stage / "README.txt").write_text(
        "Open emul.app. Copy it to Applications to install.\n" if SYSTEM == "macos" else
        "Run emul.exe. Keep the tools and licenses directories beside it.\n")
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    tar_output = dist / f"{stage.name}.tar.gz"
    tar_bundle(stage, tar_output)
    checksum(tar_output)
    if SYSTEM == "windows":
        output = dist / f"{stage.name}.zip"
        with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for file in stage.rglob("*"):
                if file.is_file():
                    archive.write(file, arcname=Path(stage.name) / file.relative_to(stage))
    else:
        output = dist / f"{stage.name}.dmg"
        run("hdiutil", "create", "-volname", "emul", "-srcfolder", stage,
            "-ov", "-format", "UDZO", output)
    checksum(output)
    print(f"Native release files: {tar_output}, {output}", flush=True)


def main():
    if SYSTEM is None:
        raise SystemExit("Use build-appimage.py for Linux bundles.")
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--prepare-media", action="store_true")
    modes.add_argument("--finish-media", action="store_true")
    options = parser.parse_args()
    specs = specifications()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(lambda item: download(CACHE, *item), specs.items()))
    if options.prepare_media:
        prepare_media(specs)
        return
    if options.finish_media:
        finish_media(specs)
        return
    if not media_ready(specs):
        if SYSTEM == "windows":
            raise RuntimeError("Build media with MSYS2 between --prepare-media and --finish-media first.")
        prepare_media(specs)
        run("sh", RECIPE, PREFIX, cwd=SOURCE)
        finish_media(specs)
    package(specs)


if __name__ == "__main__":
    main()
