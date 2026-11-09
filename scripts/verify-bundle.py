#!/usr/bin/env python3
"""Verify release archives on their native OS and architecture before publication."""
import os
from pathlib import Path
import platform
import shutil
import sys
import tarfile
import zipfile

from bundle_common import ROOT, check_architecture, digest, host_arch, run, version


def main():
    system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}[platform.system()]
    arch = host_arch()
    name = f"emul-{version()}-{system}-{arch}"
    dist = ROOT / "dist"
    verification = ROOT / "target/bundle-check" / f"{system}-{arch}"
    if verification.exists():
        shutil.rmtree(verification)
    extracted = verification / "extracted bundle"
    extracted.mkdir(parents=True)
    for checksum in dist.glob("*.sha256"):
        expected, filename = checksum.read_text().split()
        if digest(dist / filename) != expected:
            raise RuntimeError(f"Release checksum failed: {filename}")
    with tarfile.open(dist / f"{name}.tar.gz") as archive:
        archive.extractall(extracted, filter="data")
    folder = extracted / name
    suffix = ".exe" if system == "windows" else ""
    if system == "linux":
        executable = folder / "emul"
        tools = folder / "usr/bin"
        binary = tools / "emul"
    elif system == "macos":
        executable = folder / "emul.app/Contents/MacOS/emul"
        binary = executable
        tools = executable.parent / "tools"
        run("codesign", "--verify", "--strict", folder / "emul.app")
    else:
        executable = folder / "emul.exe"
        binary = executable
        tools = folder / "tools"
    check_architecture(binary, system, arch)
    for tool in ("yt-dlp", "ffmpeg", "ffprobe", "deno"):
        check_architecture(tools / f"{tool}{suffix}", system, arch)
    isolated_path = str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32") if system == "windows" else "/nonexistent"
    env = {**os.environ, "PATH": isolated_path}
    run(executable, "--check-tools", env=env)
    option = "--appdir" if system == "linux" else "--tools"
    run(sys.executable, ROOT / "tests/smoke.py", option, folder if system == "linux" else tools)
    if system == "linux":
        run(executable, "--snap", verification / "tar-window.png", "--scene", "showcase")
        image = dist / f"emul-{version()}-{arch}.AppImage"
        image_env = {**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"}
        run(image, "--check-tools", env=image_env)
        run(image, "--snap", verification / "appimage-window.png", "--scene", "showcase", env=image_env)
    elif system == "windows":
        zip_directory = verification / "zip bundle"
        with zipfile.ZipFile(dist / f"{name}.zip") as archive:
            archive.extractall(zip_directory)
        run(zip_directory / name / "emul.exe", "--check-tools", env=env)
    else:
        mount = verification / "mounted image"
        mount.mkdir()
        run("hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dist / f"{name}.dmg")
        try:
            run(mount / "emul.app/Contents/MacOS/emul", "--check-tools", env=env)
        finally:
            run("hdiutil", "detach", mount)
    print(f"Verified {system}-{arch} release bundles", flush=True)


if __name__ == "__main__":
    main()
