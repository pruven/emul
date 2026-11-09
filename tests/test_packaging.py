"""Exercise foreign executable validation and native archive layouts on any host."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import plistlib
import struct
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from bundle_common import check_architecture

spec = importlib.util.spec_from_file_location('build_native', ROOT / 'scripts/build-native.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def executable(system, arch):
    if system == 'windows':
        data = bytearray(128)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 60, 64)
        struct.pack_into('<4sH', data, 64, b'PE\0\0', 0x8664 if arch == 'x86_64' else 0xAA64)
    elif system == 'macos':
        data = bytearray(64)
        struct.pack_into('<II', data, 0, 0xFEEDFACF, 0x01000007 if arch == 'x86_64' else 0x0100000C)
    else:
        data = bytearray(64)
        data[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', data, 18, 62 if arch == 'x86_64' else 183)
    return data


class PackagingTests(unittest.TestCase):
    def test_rejects_wrong_cpu_even_when_it_can_run_under_emulation(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'program'
            for system in ('linux', 'macos', 'windows'):
                for arch, other in [('x86_64', 'aarch64'), ('aarch64', 'x86_64')]:
                    with self.subTest(system=system, arch=arch):
                        path.write_bytes(executable(system, arch))
                        check_architecture(path, system, arch)
                        with self.assertRaises(RuntimeError):
                            check_architecture(path, system, other)

    def test_universal_macos_executable_supports_both_native_bundles(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'yt-dlp'
            data = bytearray(64)
            struct.pack_into('>II', data, 0, 0xCAFEBABE, 2)
            struct.pack_into('>I', data, 8, 0x01000007)
            struct.pack_into('>I', data, 28, 0x0100000C)
            path.write_bytes(data)
            for arch in ('x86_64', 'aarch64'):
                check_architecture(path, 'macos', arch)

    def test_native_archives_preserve_tools_licenses_and_app_metadata(self):
        for system in ('macos', 'windows'):
            for arch in ('x86_64', 'aarch64'):
                with self.subTest(system=system, arch=arch), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    build = root / 'build'
                    cache, prefix, source = build / 'downloads', build / 'media', build / 'sources'
                    cache.mkdir(parents=True)
                    (prefix / 'bin').mkdir(parents=True)
                    (root / 'target/release').mkdir(parents=True)
                    (root / 'packaging').mkdir()
                    (root / 'LICENSE').write_text('Application license')
                    (root / 'packaging/THIRD-PARTY.md').write_text('Component notices')
                    (cache / 'deno-LICENSE.md').write_text('Deno license')
                    suffix = '.exe' if system == 'windows' else ''
                    payload = executable(system, arch)
                    (root / 'target/release' / f'emul{suffix}').write_bytes(payload)
                    for tool in ('ffmpeg', 'ffprobe'):
                        (prefix / 'bin' / f'{tool}{suffix}').write_bytes(payload)
                    (cache / 'yt-dlp').write_bytes(payload)
                    with zipfile.ZipFile(cache / 'deno.zip', 'w') as archive:
                        archive.writestr(f'deno{suffix}', payload)
                    with tarfile.open(cache / 'yt-dlp-source.tar.gz', 'w:gz') as archive:
                        member = tarfile.TarInfo('yt-dlp/LICENSE')
                        member.size = 7
                        archive.addfile(member, io.BytesIO(b'license'))
                    for name in ('FFmpeg-n8.0.1', 'lame-3.100'):
                        folder = source / name
                        folder.mkdir(parents=True)
                        (folder / 'COPYING').write_text('Media license')

                    def fake_run(*args, **kwargs):
                        if args[0] == 'hdiutil':
                            Path(args[-1]).write_bytes(b'fixture disk image')

                    values = dict(ROOT=root, SYSTEM=system, ARCH=arch, TARGET=f'{system}-{arch}',
                                  BUILD=build, CACHE=cache, PREFIX=prefix, SOURCE=source, SUFFIX=suffix)
                    with patch.multiple(native, **values), patch.object(native, 'run', side_effect=fake_run), patch.object(native, 'version', return_value='1.2.3'):
                        native.package({'fixture': {'sha256': 'verified'}})
                    name = f'emul-1.2.3-{system}-{arch}'
                    extracted = root / 'extracted bundle'
                    with tarfile.open(root / 'dist' / f'{name}.tar.gz') as archive:
                        archive.extractall(extracted, filter='data')
                    base = extracted / name
                    if system == 'macos':
                        app = base / 'emul.app'
                        info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
                        self.assertEqual(info['CFBundleExecutable'], 'emul')
                        self.assertEqual(info['CFBundleIdentifier'], 'org.emul.downloader')
                        program_dir = app / 'Contents/MacOS'
                        licenses = app / 'Contents/Resources/licenses'
                        if os.name != 'nt':
                            self.assertTrue((program_dir / 'emul').stat().st_mode & 0o111)
                    else:
                        program_dir, licenses = base, base / 'licenses'
                        with zipfile.ZipFile(root / 'dist' / f'{name}.zip') as archive:
                            archive.extractall(root / 'zip')
                        self.assertEqual((root / 'zip' / name / 'emul.exe').read_bytes(), payload)
                    self.assertEqual((program_dir / f'emul{suffix}').read_bytes(), payload)
                    for tool in ('yt-dlp', 'ffmpeg', 'ffprobe', 'deno'):
                        self.assertEqual((program_dir / 'tools' / f'{tool}{suffix}').read_bytes(), payload)
                    self.assertTrue((licenses / 'FFmpeg-n8.0.1-COPYING').is_file())
                    for checksum in (root / 'dist').glob('*.sha256'):
                        expected, filename = checksum.read_text().split()
                        self.assertEqual(hashlib.sha256((root / 'dist' / filename).read_bytes()).hexdigest(), expected)


if __name__ == '__main__':
    unittest.main()
