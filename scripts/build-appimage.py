#!/usr/bin/env python3
"""Build a Linux x86-64 AppImage. Requires Python 3.12+, Rust, C compiler, make,
pkg-config, patchelf, and the Rune development libraries (see README).
"""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PACKAGING = ROOT / "packaging/linux"
BUILD = ROOT / "target/appimage"
CACHE = BUILD / "downloads"
APPDIR = BUILD / "emul.AppDir"
DIST = ROOT / "dist"


def run(*args, cwd=ROOT, **kwargs):
    print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), cwd=cwd, check=True, **kwargs)


def download(item):
    name, spec = item
    path = CACHE / name
    if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != spec["sha256"]:
        print(f"Downloading {name}", flush=True)
        temp = path.with_suffix(".part")
        run("curl", "--fail", "--location", "--retry", "3", "--max-time", "300",
            "--silent", "--show-error", spec["url"], "--output", temp)
        if hashlib.sha256(temp.read_bytes()).hexdigest() != spec["sha256"]:
            temp.unlink()
            raise RuntimeError(f"SHA-256 mismatch: {name}")
        temp.replace(path)
    return path


def extract_tool(name):
    directory = BUILD / name
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir()
    tool = CACHE / f"{name}.AppImage"
    tool.chmod(0o755)
    run(tool, "--appimage-extract", cwd=directory, stdout=subprocess.DEVNULL)
    return directory / "squashfs-root/AppRun"


def main():
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise SystemExit("AppImage packaging currently supports Linux x86-64 only.")
    CACHE.mkdir(parents=True, exist_ok=True)
    DIST.mkdir(exist_ok=True)
    specs = json.loads((PACKAGING / "downloads.json").read_text())
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(download, specs.items()))

    # The stamp includes the build recipe, so recipe or dependency changes rebuild FFmpeg.
    stamp = hashlib.sha256((PACKAGING / "build-ffmpeg.sh").read_bytes()
                           + (PACKAGING / "downloads.json").read_bytes()).hexdigest()
    prefix = BUILD / "media"
    stamp_path = prefix / ".stamp"
    if not stamp_path.exists() or stamp_path.read_text() != stamp:
        source = BUILD / "media-source"
        if source.exists():
            shutil.rmtree(source)
        source.mkdir()
        if prefix.exists():
            shutil.rmtree(prefix)
        for name in ("lame", "ffmpeg"):
            with tarfile.open(CACHE / f"{name}-source.tar.gz") as archive:
                archive.extractall(source, filter="data")
        run(PACKAGING / "build-ffmpeg.sh", prefix, cwd=source)
        stamp_path.write_text(stamp)

    run("cargo", "build", "--release", "--locked")
    if APPDIR.exists():
        shutil.rmtree(APPDIR)
    bin_dir = APPDIR / "usr/bin"
    bin_dir.mkdir(parents=True)
    for name, source in [("emul", ROOT / "target/release/emul"),
                         ("ffmpeg", prefix / "bin/ffmpeg"), ("ffprobe", prefix / "bin/ffprobe")]:
        shutil.copy2(source, bin_dir / name)
        (bin_dir / name).chmod(0o755)
    with zipfile.ZipFile(CACHE / "deno.zip") as archive:
        (bin_dir / "deno").write_bytes(archive.read("deno"))
    (bin_dir / "deno").chmod(0o755)

    # Include libraries loaded by dlopen as well as ordinary ELF dependencies.
    # The Vulkan loader and graphics drivers remain on the host.
    libraries = []
    cache = subprocess.check_output(["ldconfig", "-p"], text=True)
    for name in ("libwayland-client.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1",
                 "libxkbcommon.so.0", "libxkbcommon-x11.so.0", "libfontconfig.so.1",
                 "libX11.so.6", "libXcursor.so.1", "libXrandr.so.2", "libXi.so.6"):
        matches = [line.split(" => ")[-1] for line in cache.splitlines()
                   if line.strip().startswith(name + " ") and "x86-64" in line]
        if not matches:
            raise RuntimeError(f"Missing build dependency: {name}")
        libraries.extend(["--library", matches[0]])
    run(extract_tool("linuxdeploy"), "--appdir", APPDIR,
        "--executable", bin_dir / "emul", "--executable", bin_dir / "ffmpeg",
        "--executable", bin_dir / "ffprobe", "--executable", bin_dir / "deno",
        "--desktop-file", PACKAGING / "emul.desktop", "--icon-file", PACKAGING / "emul.svg",
        "--custom-apprun", PACKAGING / "AppRun", *libraries,
        env={**os.environ, "NO_STRIP": "1"})
    # PyInstaller keeps data appended to this ELF. Copy it after linuxdeploy so
    # dependency rewriting/stripping cannot damage the embedded Python archive.
    shutil.copy2(CACHE / "yt-dlp", bin_dir / "yt-dlp")
    (bin_dir / "yt-dlp").chmod(0o755)
    notices = APPDIR / "usr/share/doc/emul"
    notices.mkdir(parents=True)
    shutil.copy2(ROOT / "LICENSE", notices / "LICENSE")
    shutil.copy2(PACKAGING / "downloads.json", notices)
    shutil.copy2(PACKAGING / "THIRD-PARTY.md", notices)
    shutil.copy2(CACHE / "deno-LICENSE.md", notices)
    system_notices = notices / "system-libraries"
    system_notices.mkdir()
    # linuxdeploy preserves Debian package copyright files automatically.
    # Debian copyright files refer to the standard license texts by path.
    if Path("/usr/share/common-licenses").exists():
        shutil.copytree("/usr/share/common-licenses", APPDIR / "usr/share/common-licenses",
                        dirs_exist_ok=True)
    # On Arch, retain notices for the packages owning the collected libraries.
    if shutil.which("pacman"):
        for library in (APPDIR / "usr/lib").glob("*.so*"):
            matches = [line.split(" => ")[-1] for line in cache.splitlines()
                       if line.strip().startswith(library.name + " ") and "x86-64" in line]
            if not matches:
                continue
            owner = subprocess.check_output(["pacman", "-Qoq", matches[0]], text=True).strip()
            license_dir = Path("/usr/share/licenses") / owner
            if license_dir.is_dir():
                shutil.copytree(license_dir, system_notices / owner, dirs_exist_ok=True)

    for directory in ("FFmpeg-n8.0.1", "lame-3.100"):
        for license_file in (BUILD / "media-source" / directory).glob("COPYING*"):
            shutil.copy2(license_file, notices / f"{directory}-{license_file.name}")
    # The standalone yt-dlp includes the Python runtime and upstream license bundle.
    with tarfile.open(CACHE / "yt-dlp-source.tar.gz") as archive:
        for member in archive.getmembers():
            if Path(member.name).name in ("LICENSE", "THIRD_PARTY_LICENSES") and member.isfile():
                (notices / f"yt-dlp-{Path(member.name).name}").write_bytes(archive.extractfile(member).read())

    run(APPDIR / "AppRun", "--check-tools")
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        version = os.environ.get("VERSION") or tomllib.load(manifest)["package"]["version"]
    version = version.removeprefix("v")
    if not version or any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-+" for c in version):
        raise RuntimeError(f"Invalid release version: {version!r}")
    output = DIST / f"emul-{version}-x86_64.AppImage"
    run(extract_tool("appimagetool"), "--runtime-file", CACHE / "runtime-x86_64",
        "--no-appstream", APPDIR, output, env={**os.environ, "ARCH": "x86_64"})
    output.chmod(0o755)
    # Publish exact FFmpeg/LAME sources and their build recipe beside the image.
    source_bundle = DIST / f"emul-{version}-media-sources.tar.gz"
    with tarfile.open(source_bundle, "w:gz") as archive:
        for name in ("ffmpeg-source.tar.gz", "lame-source.tar.gz", "yt-dlp-source.tar.gz"):
            archive.add(CACHE / name, arcname=name)
        for name in ("build-ffmpeg.sh", "downloads.json", "THIRD-PARTY.md"):
            archive.add(PACKAGING / name, arcname=name)
    for artifact in (output, source_bundle):
        (DIST / f"{artifact.name}.sha256").write_text(
            f"{hashlib.sha256(artifact.read_bytes()).hexdigest()}  {artifact.name}\n")
    print(f"AppImage: {output}", flush=True)


if __name__ == "__main__":
    main()
