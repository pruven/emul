#!/usr/bin/env python3
"""Exercise the real yt-dlp worker against generated media, without internet access."""
import functools
import http.server
import os
from pathlib import Path
import subprocess
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


with tempfile.TemporaryDirectory(prefix="tidal-fixture-") as fixture_dir:
    subprocess.run([
        "ffmpeg", "-hide_banner", "-loglevel", "error",
        "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=24",
        "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100",
        "-t", "1", "-c:v", "libx264", "-c:a", "aac", "-shortest",
        str(Path(fixture_dir) / "fixture.mp4"),
    ], check=True)
    Path(fixture_dir, "en.vtt").write_text("WEBVTT\n\n00:00.000 --> 00:01.000\nA local subtitle fixture.\n")
    Path(fixture_dir, "subtitles.html").write_text('<html><head><title>Subtitle fixture</title></head><body><video controls><source src="fixture.mp4" type="video/mp4"><track kind="subtitles" src="en.vtt" srclang="en" label="English"></video></body></html>')
    handler = functools.partial(QuietHandler, directory=fixture_dir)
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            subprocess.run([
                "cargo", "test", "--locked", "--manifest-path", "Cargo.toml",
                "downloads_real_media", "--", "--ignored", "--nocapture",
            ], cwd=ROOT, env={
                **os.environ,
                "TIDAL_TEST_MEDIA_URL": f"http://127.0.0.1:{server.server_port}/fixture.mp4",
                "TIDAL_TEST_SUBTITLE_URL": f"http://127.0.0.1:{server.server_port}/subtitles.html",
            }, check=True)
        finally:
            server.shutdown()
