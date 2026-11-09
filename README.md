# Tidal

A native media downloader built with [Rune](https://github.com/anomalyco/rune) and [yt-dlp](https://github.com/yt-dlp/yt-dlp), inspired by ID4's focused download workflow. Rust renders the entire interface; there is no webview or local web server.

![Native Rune interface](shots/showcase.png)

## Run

Install Rust, yt-dlp, and FFmpeg. Make sure `yt-dlp` and `ffmpeg` are available on PATH **when launching the app**.

```sh
# From /home/himmel/projects/tidal
cargo run
# Optimized standalone executable
cargo build --release --locked
```

The executable is `target/release/tidal-downloader` (`.exe` on Windows). Rune is pre-release and pinned to commit `dece2aff3a254ffd6f6a0a49baa82b75664b3190` for reproducible dependency resolution.

Linux also needs `libxkbcommon` and `libwayland-client` development libraries to build, and a Vulkan driver to render. On Ubuntu/Debian:

```sh
sudo apt-get install libxkbcommon-dev libwayland-dev libxkbcommon-x11-0 mesa-vulkan-drivers
```

On macOS, install Xcode Command Line Tools; Homebrew provides yt-dlp and FFmpeg. On Windows, install the Rust MSVC toolchain and Visual Studio C++ Build Tools; install yt-dlp and FFmpeg and add them to PATH. An installed GPU driver is required. The included CI workflow builds Linux, macOS, and Windows executables; only Linux has been exercised locally.

## Use

1. Paste a complete HTTP(S) link, or use **Paste link**.
2. Select **Video**, **Audio**, or **Subtitles**.
3. Click the video quality control to cycle **1080p → 4K → Best available → 720p**. Quality is an upper bound; the source determines available formats.
4. Click **Download** or press Enter in the URL field.

Video preserves source codecs; split streams merge into MKV, while a single stream can retain its original container. Audio extracts to MP3 at yt-dlp's best encoding quality. Subtitles download English manual or automatic tracks as SRT when available; an absent track is reported as an error. This version saves one item per link; it does not expand playlists.

The queue runs one download at a time, oldest first. Cancel stops yt-dlp and its FFmpeg child processes; partial files stay in the destination so **Retry** can resume where yt-dlp supports it. Completed files appear under **Completed**, with **Open folder**. **Clear finished** clears completed, failed, and cancelled history entries; it leaves downloaded files on disk.

Settings accepts an absolute destination path. It creates the folder on save and applies changes to future jobs. Defaults to `Downloads/Tidal`. History and the selected folder live in the platform's local application data directory under `tidal-downloader/`. Closing the app cancels running work; interrupted entries offer Retry on the next launch. No accounts, analytics, or cloud persistence. Media providers receive ordinary yt-dlp network requests.

## Verify

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run -- --snap /tmp/tidal.png --scene showcase
```

The `showcase` scene uses illustrative queue content for design inspection only; it never downloads or saves history. Ordinary launches show your actual queue and history. Rune's built-in `--snap`, `--tree`, and `--script` tools render and drive the native interface headlessly without a display.

Tests exercise URL validation, shell-free argument construction, progress decoding, history recovery, format selection, queueing, cancellation, retry, and navigation. An optional real worker test generates a short fixture with FFmpeg, serves it locally, and verifies video, MP3, SRT conversion, missing-subtitle errors, and process cancellation:

```sh
python3 tests/smoke.py
```

Use media you own or have permission to download. Availability and provider authentication restrictions are determined by yt-dlp. Cookie management, playlist expansion, built-in dependency installation, signed installers, automatic updates, and mobile targets are not included.
