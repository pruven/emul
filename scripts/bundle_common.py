"""Shared release helpers; native builds only, with verified dependency downloads."""
import hashlib
import os
from pathlib import Path
import platform
import struct
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def host_arch():
    machine = platform.machine().lower()
    aliases = {"amd64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}
    if machine not in aliases:
        raise RuntimeError(f"Unsupported desktop architecture: {machine}")
    return aliases[machine]


def version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        result = os.environ.get("VERSION") or tomllib.load(manifest)["package"]["version"]
    result = result.removeprefix("v")
    if not result or any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-+" for c in result):
        raise ValueError(f"Invalid release version: {result!r}")
    return result


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(*args, **kwargs):
    print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True, **kwargs)


def download(directory, name, spec):
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / name
    if path.exists() and digest(path) == spec["sha256"]:
        return path
    temporary = directory / f"{name}.part"
    run("curl", "--fail", "--location", "--retry", "3", "--max-time", "300",
        "--silent", "--show-error", spec["url"], "--output", temporary)
    if digest(temporary) != spec["sha256"]:
        temporary.unlink()
        raise RuntimeError(f"SHA-256 mismatch: {name}")
    temporary.replace(path)
    return path


def checksum(path):
    path.with_name(f"{path.name}.sha256").write_text(f"{digest(path)}  {path.name}\n")


def tar_bundle(directory, output):
    with tarfile.open(output, "w:gz") as archive:
        archive.add(directory, arcname=directory.name)


def check_architecture(path, system, arch):
    """Reject accidentally emulated or cross-architecture payloads before publishing."""
    with Path(path).open("rb") as stream:
        header = stream.read(64)
        if system == "windows":
            if header[:2] != b"MZ":
                raise RuntimeError(f"Not a Windows executable: {path}")
            stream.seek(struct.unpack_from("<I", header, 60)[0])
            signature, machine = struct.unpack("<4sH", stream.read(6))
            valid = signature == b"PE\0\0" and machine == {"x86_64": 0x8664, "aarch64": 0xAA64}[arch]
        elif system == "macos":
            expected = {"x86_64": 0x01000007, "aarch64": 0x0100000C}[arch]
            magic = struct.unpack_from(">I", header)[0]
            if magic in (0xCAFEBABE, 0xCAFEBABF):
                count = struct.unpack_from(">I", header, 4)[0]
                stream.seek(8)
                entries = [stream.read(32 if magic == 0xCAFEBABF else 20) for _ in range(count)]
                valid = any(struct.unpack_from(">I", entry)[0] == expected for entry in entries)
            else:
                valid = header[:4] == b"\xcf\xfa\xed\xfe" and struct.unpack_from("<I", header, 4)[0] == expected
        else:
            valid = (header[:5] == b"\x7fELF\x02" and header[5] == 1
                     and struct.unpack_from("<H", header, 18)[0] == {"x86_64": 62, "aarch64": 183}[arch])
    if not valid:
        raise RuntimeError(f"Executable architecture does not match {system}-{arch}: {path}")
