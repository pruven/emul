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
