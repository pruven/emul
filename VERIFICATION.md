# Local verification

Verified on Linux, November 9, 2026, using Rust 1.97.0, the installed yt-dlp and FFmpeg, and Intel UHD Graphics 630 with Vulkan.

- Native debug build: passed.
- Rust tests: 8 passed; 1 real-worker test is opt-in.
- Real-worker test via `python3 tests/smoke.py`: passed. Downloaded generated video over localhost, extracted MP3, converted a real VTT track into SRT, reported missing subtitles, and cancelled a worker process.
- Clippy with warnings denied: passed.
- Rustfmt and whitespace checks: passed.
- Rune headless native renders inspected at 1180×820 and the 980×720 minimum; settings and completed navigation driven by Rune input scripts.

Screenshots in `shots/` use illustrative queue content through the explicit `showcase` scene. The normal app opens with real local history. All UI is rendered by Rune; website-specific HTML/browser design checkers do not apply.

The design uses dark green charcoal surfaces, a mint accent, Rune's embedded Inter, restrained borders, and native hover/press responses. Icons are local vector paths; no external photographs, font requests, or asset placeholders are used.

Linux is the locally verified target. The CI workflow defines builds for Linux, macOS, and Windows; it has not run in this local session. Signed installers and live provider-specific downloads have not been verified.

## Bundled Linux AppImage

Verified locally on November 9, 2026:

- Built `dist/emul-0.1.0-x86_64.AppImage` with yt-dlp 2026.10.26,
  FFmpeg/ffprobe 8.0.1, and Deno 2.9.8.
- Extracted the actual AppImage and checked all four tools with
  `PATH=/nonexistent`; all passed.
- Ran `python3 tests/smoke.py --appdir target/appimage-verify/squashfs-root`:
  video download, MP3 extraction, VTT-to-SRT conversion, missing-subtitle
  errors, and cancellation passed using the bundled media tools.
- Executed the actual AppImage with `APPIMAGE_EXTRACT_AND_RUN=1`;
  `--check-tools` and native Vulkan rendering through `--snap` passed.
- Rust tests: 8 passed; the opt-in real-worker test passed separately.
  Clippy, Rustfmt, Python syntax, shell syntax, and whitespace checks passed.
- The local artifact was built on this machine (glibc 2.44). Compatibility
  with older distributions has not been verified locally. The release workflow
  builds on Ubuntu 24.04 to establish a glibc 2.39 baseline.
- The tag-triggered GitHub Release workflow is configured but has not run;
  no GitHub repository exists yet, so nothing has been published.

The AppImage uses bundled media tools and desktop libraries. The graphics driver
and Vulkan loader remain on the host. Exact FFmpeg/LAME source archives, the
yt-dlp source distribution, the media build recipe, and download checksums are
included in the companion media source archive. Release SHA-256 files are
written alongside each artifact.

## Portable Linux tar archive

Built `dist/emul-0.1.0-linux-x86_64.tar.gz` with the same media tools and desktop
libraries as the AppImage. Extracted it into a path containing spaces and verified
the top-level `emul` launcher with no system executables on PATH. Bundled media
download/conversion tests, native Vulkan rendering, executable permissions, and
all release checksums passed. The release workflow now verifies and publishes
this runnable archive and its checksum alongside the AppImage and media sources.

## Six-target desktop release workflow

Configured native builds for Linux, macOS, and Windows on x86-64 and ARM64.
The unified `.github/workflows/release.yml` replaces the previous native and
Linux-only workflows. Every target creates a runnable tar archive; Linux adds
AppImage, macOS adds an ad-hoc-signed app in a DMG, and Windows adds a ZIP.
A pushed matching `v*` tag publishes only after all six matrix builds pass.

Locally verified on Linux x86-64:

- Rust tests: 8 passed; Clippy with warnings denied and Rustfmt passed.
- Packaging tests: wrong-CPU rejection, universal Mach-O acceptance, and archive
  layout/metadata/checksums for both Windows and macOS architectures passed.
  These layout tests use synthetic executable headers, not native builds.
- Actionlint v1.7.12 accepted the complete six-target workflow.
- Rebuilt the real Linux AppImage/tar, verified CPU headers and checksums,
  checked bundled tools with host PATH removed, ran the real download/audio/
  subtitle worker test, and rendered both packages through Vulkan successfully.
- Official upstream release manifests supplied pinned yt-dlp, Deno, and AppImage
  tools for each supported architecture. FFmpeg/LAME sources remain shared.

Windows, macOS, and Linux ARM64 executables cannot be executed on this local
Linux x86-64 host and have not been built or run here. Their native runner jobs
validate executable headers, tool startup, real media operations, and the
produced tar plus ZIP/DMG/AppImage as appropriate before release publication.
Native GPU rendering is verified locally and in Linux CI; macOS/Windows GUI
rendering is not claimed by the hosted-runner tool checks. Apple notarization,
Developer ID signatures, Windows certificate signing, and 32-bit/other CPU
architectures are not configured.
