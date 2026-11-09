use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Format {
    Video,
    Audio,
    Subtitles,
}
impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Subtitles => "Subtitles",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Queued,
    Running,
    Complete,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub format: Format,
    pub quality: u32,
    pub directory: PathBuf,
    pub status: Status,
    pub progress: f32,
    pub detail: String,
    pub file: Option<PathBuf>,
}
#[derive(Debug)]
pub enum Event {
    Line(u64, String),
    End(u64, Result<(), String>),
}

pub fn validate_url(input: &str) -> Result<String, String> {
    let input = input.trim();
    let parsed = url::Url::parse(input)
        .map_err(|_| "Paste a complete http or https media link.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Use an http or https link without embedded credentials.".into());
    }
    Ok(parsed.to_string())
}

pub fn arguments(job: &Job) -> Vec<String> {
    let mut args: Vec<String> = [
        "--ignore-config",
        "--no-playlist",
        "--no-colors",
        "--newline",
        "--progress",
        "--no-simulate",
        "--restrict-filenames",
        "--windows-filenames",
        "--print",
        "before_dl:__meta:%(title)j",
        "--print",
        "after_move:__file:%(filepath)j",
        "--progress-template",
        "download:__progress:%(progress)j",
        "--output",
        "%(title).160B [%(id)s].%(ext)s",
        "--paths",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(job.directory.to_string_lossy().into_owned());
    match job.format {
        Format::Video => {
            let selector = if job.quality == 0 {
                "bv*+ba/b".to_string()
            } else {
                format!(
                    "bv*[height<=?{}]+ba/b[height<=?{}]",
                    job.quality, job.quality
                )
            };
            args.extend([
                "-f".into(),
                selector,
                "--merge-output-format".into(),
                "mkv".into(),
            ]);
        }
        Format::Audio => {
            args.extend(["-x", "--audio-format", "mp3", "--audio-quality", "0"].map(String::from))
        }
        Format::Subtitles => args.extend(
            [
                "--skip-download",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "en.*",
                "--sub-format",
                "srt/best",
                "--convert-subs",
                "srt",
                "--print",
                "after_video:__subs:%(requested_subtitles)j",
            ]
            .map(String::from),
        ),
    }
    args.extend(["--".into(), job.url.clone()]);
    args
}

pub fn spawn(job: Job, tx: Sender<Event>, cancel: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let result = run(&job, &tx, &cancel);
        let _ = tx.send(Event::End(job.id, result));
    })
}
fn run(job: &Job, tx: &Sender<Event>, cancel: &AtomicBool) -> Result<(), String> {
    std::fs::create_dir_all(&job.directory)
        .map_err(|e| format!("Cannot create download folder: {e}"))?;
    let mut command = Command::new("yt-dlp");
    command
        .args(arguments(job))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start yt-dlp. Install it and add it to PATH. {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let sender = tx.clone();
    let id = job.id;
    let out = thread::spawn(move || {
        let mut subtitles = false;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(json) = line.strip_prefix("__subs:") {
                subtitles = serde_json::from_str::<serde_json::Value>(json)
                    .ok()
                    .and_then(|v| v.as_object().map(|m| !m.is_empty()))
                    .unwrap_or(false);
            }
            let _ = sender.send(Event::Line(id, line));
        }
        subtitles
    });
    let err = thread::spawn(move || {
        let mut tail = VecDeque::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tail.len() == 6 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            // Stop the entire process group, including an FFmpeg postprocessor.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .output();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(80)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    };
    let subs = out.join().unwrap_or(false);
    let error = err.join().unwrap_or_default();
    if !status.success() {
        return Err(if error.is_empty() {
            format!("yt-dlp exited with {status}")
        } else {
            error
        });
    }
    if job.format == Format::Subtitles && !subs {
        return Err("No English subtitles are available for this media.".into());
    }
    Ok(())
}

pub fn apply_line(job: &mut Job, line: &str) {
    if let Some(value) = line
        .strip_prefix("__meta:")
        .and_then(|s| serde_json::from_str::<String>(s).ok())
    {
        job.title = value;
    }
    if let Some(value) = line
        .strip_prefix("__file:")
        .and_then(|s| serde_json::from_str::<String>(s).ok())
    {
        job.file = Some(value.into());
    }
    if let Some(value) = line
        .strip_prefix("__progress:")
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
    {
        let downloaded = value["downloaded_bytes"].as_f64().unwrap_or(0.0);
        let total = value["total_bytes"]
            .as_f64()
            .or(value["total_bytes_estimate"].as_f64())
            .unwrap_or(0.0);
        job.progress = if total > 0.0 {
            (downloaded / total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let speed = value["speed"].as_f64().unwrap_or(0.0) / 1_000_000.0;
        let eta = value["eta"].as_u64();
        job.detail = if value["status"] == "finished" {
            "Processing media…".into()
        } else {
            format!(
                "{speed:.1} MB/s{}",
                eta.map(|t| format!("  ·  {t}s left")).unwrap_or_default()
            )
        };
    }
}

pub fn history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("emul/history.json")
}
pub fn load_history(path: &Path) -> Vec<Job> {
    let mut jobs: Vec<Job> = std::fs::read(path)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or_default();
    for job in &mut jobs {
        if matches!(job.status, Status::Running | Status::Queued) {
            job.status = Status::Cancelled;
            job.detail = "Interrupted when the app closed. Retry to resume.".into();
        }
    }
    jobs
}
pub fn save_history(path: &Path, jobs: &[Job]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Finish writing before replacing the last valid history.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(jobs)?)?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job() -> Job {
        Job {
            id: 1,
            url: "https://example.com/video?a=1&b=2".into(),
            title: "Video".into(),
            format: Format::Video,
            quality: 1080,
            directory: "/tmp/media folder".into(),
            status: Status::Queued,
            progress: 0.0,
            detail: String::new(),
            file: None,
        }
    }
    #[test]
    fn accepts_only_web_urls() {
        for s in [
            "file:///etc/passwd",
            "--exec=bad",
            "javascript:alert(1)",
            "https://user:pass@site.com",
        ] {
            assert!(validate_url(s).is_err());
        }
        assert!(validate_url(" https://example.com/a ").is_ok());
    }
    #[test]
    fn arguments_preserve_url_and_directory() {
        let j = job();
        let a = arguments(&j);
        assert_eq!(a.last(), Some(&j.url));
        assert_eq!(a[a.len() - 2], "--");
        assert!(a.contains(&"/tmp/media folder".into()));
        assert!(a.contains(&"bv*[height<=?1080]+ba/b[height<=?1080]".into()));
    }
    #[test]
    fn parses_progress_and_metadata() {
        let mut j = job();
        apply_line(&mut j, "__meta:\"A real title\"");
        apply_line(
            &mut j,
            "__progress:{\"downloaded_bytes\":50,\"total_bytes\":100,\"speed\":2000000,\"eta\":3}",
        );
        assert_eq!(j.title, "A real title");
        assert_eq!(j.progress, 0.5);
        assert!(j.detail.contains("2.0 MB/s"));
    }
    #[test]
    fn subtitle_mode_never_downloads_video() {
        let mut j = job();
        j.format = Format::Subtitles;
        let args = arguments(&j);
        assert!(args.contains(&"--skip-download".into()));
        assert!(args.contains(&"--write-auto-subs".into()));
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    #[test]
    fn history_survives_and_interrupted_jobs_require_explicit_retry() {
        let path =
            std::env::temp_dir().join(format!("emul-history-test-{}.json", std::process::id()));
        let job = Job {
            id: 1,
            url: "https://example.com/video".into(),
            title: "Keep me".into(),
            format: Format::Video,
            quality: 1080,
            directory: std::env::temp_dir(),
            status: Status::Running,
            progress: 0.5,
            detail: String::new(),
            file: None,
        };
        save_history(&path, &[job]).unwrap();
        let loaded = load_history(&path);
        assert_eq!(loaded[0].status, Status::Cancelled);
        assert_eq!(loaded[0].title, "Keep me");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "Requires a local HTTP media fixture at EMUL_TEST_MEDIA_URL and yt-dlp/FFmpeg"]
    fn downloads_real_media() {
        let url = std::env::var("EMUL_TEST_MEDIA_URL").expect("Set fixture URL");
        let directory =
            std::env::temp_dir().join(format!("emul-download-test-{}", std::process::id()));
        for (id, format) in [(1, Format::Video), (2, Format::Audio)] {
            let job = Job {
                id,
                url: url.clone(),
                title: "Fixture".into(),
                format,
                quality: 1080,
                directory: directory.clone(),
                status: Status::Running,
                progress: 0.,
                detail: String::new(),
                file: None,
            };
            let (tx, rx) = std::sync::mpsc::channel();
            spawn(job, tx, Arc::new(AtomicBool::new(false)));
            let mut file = None;
            loop {
                match rx.recv_timeout(Duration::from_secs(60)).unwrap() {
                    Event::End(_, result) => {
                        result.unwrap();
                        break;
                    }
                    Event::Line(_, line) => {
                        if let Some(value) = line
                            .strip_prefix("__file:")
                            .and_then(|j| serde_json::from_str::<String>(j).ok())
                        {
                            file = Some(PathBuf::from(value));
                        }
                    }
                }
            }
            let file = file.expect("Worker must report output path");
            assert!(file.metadata().unwrap().len() > 0);
            if format == Format::Audio {
                assert_eq!(file.extension().unwrap(), "mp3");
            }
        }
        for (id, format, cancelled) in [(3, Format::Subtitles, false), (4, Format::Video, true)] {
            let job = Job {
                id,
                url: url.clone(),
                title: "Fixture".into(),
                format,
                quality: 1080,
                directory: directory.clone(),
                status: Status::Running,
                progress: 0.,
                detail: String::new(),
                file: None,
            };
            let (tx, rx) = std::sync::mpsc::channel();
            spawn(job, tx, Arc::new(AtomicBool::new(cancelled)));
            loop {
                if let Event::End(_, result) = rx.recv_timeout(Duration::from_secs(60)).unwrap() {
                    let error = result.expect_err(
                        "Unavailable subtitles and cancellation must not report success",
                    );
                    assert!(
                        error.contains(if cancelled {
                            "Cancelled"
                        } else {
                            "No English subtitles"
                        }),
                        "{error}"
                    );
                    break;
                }
            }
        }
        if let Ok(subtitle_url) = std::env::var("EMUL_TEST_SUBTITLE_URL") {
            let job = Job {
                id: 5,
                url: subtitle_url,
                title: "Subtitles".into(),
                format: Format::Subtitles,
                quality: 1080,
                directory: directory.clone(),
                status: Status::Running,
                progress: 0.,
                detail: String::new(),
                file: None,
            };
            let (tx, rx) = std::sync::mpsc::channel();
            spawn(job, tx, Arc::new(AtomicBool::new(false)));
            loop {
                if let Event::End(_, result) = rx.recv_timeout(Duration::from_secs(60)).unwrap() {
                    result.unwrap();
                    break;
                }
            }
            let srt = std::fs::read_dir(&directory)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|p| p.extension().is_some_and(|ext| ext == "srt"))
                .expect("Subtitle worker must produce an SRT file");
            assert!(
                std::fs::read_to_string(srt)
                    .unwrap()
                    .contains("A local subtitle fixture.")
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
