The emul AppImage includes separate executables for yt-dlp, FFmpeg, ffprobe,
and Deno. They are invoked as subprocesses; emul does not link against FFmpeg.

- yt-dlp 2026.10.26: https://github.com/yt-dlp/yt-dlp/tree/2026.10.26
  The Linux standalone executable includes Python and other dependencies.
  Its upstream license notices are included in this directory and the executable.
  See https://github.com/yt-dlp/yt-dlp#license for binary licensing details.
- FFmpeg 8.0.1: https://github.com/FFmpeg/FFmpeg/tree/n8.0.1
  Built from source without GPL/nonfree components, with LGPL LAME support.
  FFmpeg's COPYING files are included in this directory.
- LAME 3.100: https://lame.sourceforge.io/ (LGPL)
  LAME's COPYING file is included in this directory.
- Deno 2.9.8: https://github.com/denoland/deno/tree/v2.9.8 (MIT)
  Provides yt-dlp's JavaScript runtime for sites such as YouTube.
- Runtime libraries are collected by linuxdeploy from the Linux build host.
  Their upstream copyright notices are included under ../ (Debian/Ubuntu)
  or system-libraries/ (Arch).

The media-sources.tar.gz release asset includes the exact FFmpeg and LAME
source archives, the yt-dlp source distribution, the download manifest with
SHA-256 checksums, and the FFmpeg/LAME build recipe. To rebuild those media
binaries, extract both media source archives into one directory and run:

    ./build-ffmpeg.sh /absolute/path/to/install-prefix

Use a C compiler, make, and pkg-config on Linux. The default build uses four
jobs; set CARGO_BUILD_JOBS to change it. GPL and nonfree FFmpeg configure flags
are intentionally not enabled. All notices apply to their respective components.
