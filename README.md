# emul

A native media downloader built with [Rune](https://github.com/anomalyco/rune) and [yt-dlp](https://github.com/yt-dlp/yt-dlp), inspired by ID4's focused download workflow. Rust renders the entire interface; there is no webview or local web server.

![Native Rune interface](shots/showcase.png)

## Linux AppImage

Download `emul-<version>-x86_64.AppImage` from the repository's GitHub Releases,
make it executable, and run it:

```sh
chmod +x emul-*-x86_64.AppImage
./emul-*-x86_64.AppImage
```

The AppImage bundles yt-dlp (including Python and the EJS scripts), FFmpeg,
ffprobe, and Deno. No separate media-tool installation is needed. Bundled tools
are preferred over system tools. Linux x86-64 with glibc 2.39 or newer
(Ubuntu 24.04 or equivalent) and a working Vulkan graphics driver is required.
The window supports Wayland and X11. ARM Linux is not packaged yet.

If FUSE is unavailable, launch with `APPIMAGE_EXTRACT_AND_RUN=1`.
Use `./emul-<version>-x86_64.AppImage --check-tools` to check bundled tools.
Updating the AppImage updates its media tools; bundled tools do not self-update.

## Linux executable archive

Releases also include `emul-<version>-linux-x86_64.tar.gz`, with the same bundled
media tools and desktop libraries. Extract it and run the top-level `emul` launcher:

```sh
tar -xzf emul-0.1.0-linux-x86_64.tar.gz
./emul-0.1.0-linux-x86_64/emul
```

Keep the extracted directory together. This format needs no FUSE and has the same
Linux and graphics requirements as the AppImage. The separate `media-sources`
archive contains source code, not the runnable application.

## Run from source

Install Rust, yt-dlp, and FFmpeg. Install Deno for YouTube JavaScript challenges
(see [yt-dlp’s runtime setup](https://github.com/yt-dlp/yt-dlp/wiki/EJS)). Make sure `yt-dlp` and `ffmpeg` are available on PATH **when launching the app**.

```sh
# From the project root
cargo run
# Optimized standalone executable
cargo build --release --locked
```

The executable is `target/release/emul` (`.exe` on Windows). Rune is pre-release and pinned to commit `dece2aff3a254ffd6f6a0a49baa82b75664b3190` for reproducible dependency resolution.

Linux also needs `libxkbcommon` and `libwayland-client` development libraries to build, and a Vulkan driver to render. On Ubuntu/Debian:

```sh
sudo apt-get install libxkbcommon-dev libwayland-dev libxkbcommon-x11-0 mesa-vulkan-drivers
```

On macOS, install Xcode Command Line Tools; Homebrew provides yt-dlp and FFmpeg. On Windows, install the Rust MSVC toolchain and Visual Studio C++ Build Tools; install yt-dlp and FFmpeg and add them to PATH. An installed GPU driver is required. The native CI workflow builds Linux, macOS, and Windows executables. AppImage packaging is Linux-only; only Linux has been exercised locally.

## Use

1. Paste a complete HTTP(S) link, or use **Paste link**.
2. Select **Video**, **Audio**, or **Subtitles**.
3. Click the video quality control to cycle **1080p → 4K → Best available → 720p**. Quality is an upper bound; the source determines available formats.
4. Click **Download** or press Enter in the URL field.

Video preserves source codecs; split streams merge into MKV, while a single stream can retain its original container. Audio extracts to MP3 at yt-dlp's best encoding quality. Subtitles download English manual or automatic tracks as SRT when available; an absent track is reported as an error. This version saves one item per link; it does not expand playlists.

The queue runs one download at a time, oldest first. Cancel stops yt-dlp and its FFmpeg child processes; partial files stay in the destination so **Retry** can resume where yt-dlp supports it. Completed files appear under **Completed**, with **Open folder**. **Clear finished** clears completed, failed, and cancelled history entries; it leaves downloaded files on disk.

Settings accepts an absolute destination path. It creates the folder on save and applies changes to future jobs. Defaults to `Downloads/emul`. History and the selected folder live in the platform's local application data directory under `emul/`. Closing the app cancels running work; interrupted entries offer Retry on the next launch. No accounts, analytics, or cloud persistence. Media providers receive ordinary yt-dlp network requests.

## Build and release Linux bundles

On Linux x86-64, install Python 3.12+, Rust, a C compiler, make, curl, pkg-config,
patchelf, and the Linux libraries listed above (also `libfontconfig1`, `libx11-6`,
`libxcursor1`, `libxrandr2`, and `libxi6`). Then run:

```sh
python3 scripts/build-appimage.py
```

The script verifies pinned SHA-256 downloads, builds FFmpeg with LAME for MP3
encoding, gathers desktop libraries, and writes the AppImage, executable `.tar.gz`,
media source archive, and checksum files to `dist/`. Download and media build caches live under
`target/appimage/`. Build on Ubuntu 24.04 for the documented glibc baseline;
building on a newer distribution can raise the minimum required glibc version.

The `Linux bundles` workflow validates pull requests and accepts manual builds.
Pushing a version tag matching `Cargo.toml` (for example `v0.1.0`) builds, tests,
and publishes those files to a GitHub Release. Manual builds upload Actions
artifacts without publishing. The release includes component notices and the
exact media source archives with their build recipe. See
[third-party components](packaging/linux/THIRD-PARTY.md).

## Verify

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run -- --snap /tmp/emul.png --scene showcase
```

The `showcase` scene uses illustrative queue content for design inspection only; it never downloads or saves history. Ordinary launches show your actual queue and history. Rune's built-in `--snap`, `--tree`, and `--script` tools render and drive the native interface headlessly without a display.

Tests exercise URL validation, shell-free argument construction, progress decoding, history recovery, format selection, queueing, cancellation, retry, and navigation. An optional real worker test generates a short fixture with FFmpeg, serves it locally, and verifies video, MP3, SRT conversion, missing-subtitle errors, and process cancellation:

```sh
python3 tests/smoke.py
```

Use media you own or have permission to download. Availability and provider authentication restrictions are determined by yt-dlp. Cookie management, playlist expansion, built-in dependency installation for source builds, signed installers, automatic updates, and mobile targets are not included.
