mod engine;
use engine::{Event, Format, Job, Status};
use rune::{
    AppSpec, Color, Cx, El, Latest, ScrollSpec, Shape, TextEdit, Tx, canvas, col, el, row, text,
    text_input,
    tok::{TextStyle, motion},
};
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

fn c(hex: &str) -> Color {
    rune::color::hex(hex).unwrap()
}
fn ink() -> Color {
    c("#eef2f0")
}
fn muted() -> Color {
    c("#96a39e")
}
fn mint() -> Color {
    c("#abedc6")
}
fn line() -> Color {
    c("#29332e")
}
fn label(value: impl Into<String>, size: f32, weight: f32, color: Color) -> El {
    text(value).font(TextStyle::new(size, weight)).color(color)
}
fn space() -> El {
    el().grow(1.0)
}
fn rule() -> El {
    el().w_full().h(1.0).fill(line())
}
fn button(key: impl Into<String>, title: impl Into<String>, primary: bool) -> El {
    row()
        .key(key)
        .role("button")
        .focusable(true)
        .center()
        .gap(9.0)
        .h(40.0)
        .px(16.0)
        .radius(8.0)
        .fill(if primary { mint() } else { c("#202923") })
        .border(1.0, if primary { mint() } else { line() })
        .hover(c("#ffffff").alpha(0.07))
        .press_scale(0.98)
        .motion(motion::SNAPPY)
        .child(label(
            title,
            13.0,
            600.0,
            if primary { c("#112219") } else { ink() },
        ))
}
fn glyph(name: &str, size: f32, color: Color) -> El {
    let paths: Vec<Vec<[f32; 2]>> = match name {
        "download" => vec![
            vec![[12., 3.], [12., 15.]],
            vec![[7., 10.], [12., 15.], [17., 10.]],
            vec![[4., 16.], [4., 21.], [20., 21.], [20., 16.]],
        ],
        "video" => vec![
            vec![[3., 5.], [16., 5.], [16., 19.], [3., 19.], [3., 5.]],
            vec![[16., 9.], [22., 6.], [22., 18.], [16., 15.]],
        ],
        "audio" => vec![
            vec![[9., 18.], [9., 5.], [20., 3.], [20., 16.]],
            vec![[9., 8.], [20., 6.]],
            vec![
                [9., 16.],
                [4., 16.],
                [3., 18.],
                [4., 21.],
                [7., 21.],
                [9., 18.],
            ],
            vec![[20., 14.], [16., 14.], [15., 17.], [18., 19.], [20., 16.]],
        ],
        "subs" => vec![
            vec![[3., 5.], [21., 5.], [21., 19.], [3., 19.], [3., 5.]],
            vec![[6., 11.], [10., 11.]],
            vec![[13., 11.], [18., 11.]],
            vec![[6., 15.], [15., 15.]],
        ],
        "folder" => vec![vec![
            [3., 7.],
            [3., 20.],
            [21., 20.],
            [21., 7.],
            [12., 7.],
            [10., 4.],
            [3., 4.],
            [3., 7.],
        ]],
        "check" => vec![vec![[5., 12.], [10., 17.], [20., 6.]]],
        "close" => vec![vec![[6., 6.], [18., 18.]], vec![[18., 6.], [6., 18.]]],
        "link" => vec![
            vec![[9., 15.], [15., 9.]],
            vec![
                [8., 11.],
                [5., 14.],
                [5., 18.],
                [8., 21.],
                [11., 21.],
                [14., 18.],
            ],
            vec![
                [10., 6.],
                [13., 3.],
                [17., 3.],
                [20., 6.],
                [20., 10.],
                [17., 13.],
            ],
        ],
        _ => vec![
            vec![[5., 7.], [19., 7.]],
            vec![[5., 12.], [19., 12.]],
            vec![[5., 17.], [15., 17.]],
        ],
    };
    let mut shape = Shape::new().view_box(24., 24.);
    for points in paths {
        shape = shape.stroke(points, 1.6, color);
    }
    canvas(shape).size(size, size)
}

struct State {
    input: TextEdit,
    folder: TextEdit,
    directory: PathBuf,
    format: Format,
    quality: u32,
    tab: u8,
    settings: bool,
    jobs: Vec<Job>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    active: Option<(u64, Arc<AtomicBool>)>,
    worker: Option<std::thread::JoinHandle<()>>,
    tools: Latest<(bool, bool)>,
    ready: Option<(bool, bool)>,
    error: String,
    demo: bool,
}
impl State {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let folder = std::fs::read_to_string(engine::history_path().with_file_name("folder.txt"))
            .ok()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| {
                dirs::download_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("emul")
                    .to_string_lossy()
                    .into_owned()
            });
        Self {
            input: TextEdit::new(""),
            directory: PathBuf::from(&folder),
            folder: TextEdit::new(folder),
            format: Format::Video,
            quality: 1080,
            tab: 0,
            settings: false,
            jobs: engine::load_history(&engine::history_path()),
            tx,
            rx,
            active: None,
            worker: None,
            tools: Latest::default(),
            ready: None,
            error: String::new(),
            demo: false,
        }
    }
    fn save(&mut self) {
        if self.demo {
            return;
        }
        if let Err(e) = engine::save_history(&engine::history_path(), &self.jobs) {
            self.error = format!("Could not save download history: {e}");
        }
    }
    fn enqueue(&mut self) {
        let url = match engine::validate_url(&self.input.text) {
            Ok(url) => url,
            Err(e) => {
                self.error = e;
                return;
            }
        };
        if self.ready != Some((true, true)) {
            self.error =
                "Media tools are missing. Reinstall the AppImage, or install yt-dlp and FFmpeg on PATH.".into();
            return;
        }
        let directory = self.directory.clone();
        if !directory.is_absolute() {
            self.error = "Choose an absolute download folder in Settings.".into();
            self.settings = true;
            return;
        }
        let id = self.jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
        let title = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| url.clone());
        self.jobs.insert(
            0,
            Job {
                id,
                url,
                title,
                format: self.format,
                quality: self.quality,
                directory,
                status: Status::Queued,
                progress: 0.0,
                detail: "Waiting in queue".into(),
                file: None,
            },
        );
        self.error.clear();
        self.input.clear();
        self.tab = 0;
        self.save();
    }
    fn cancel(&mut self, id: u64) {
        if let Some((active, flag)) = &self.active
            && *active == id
        {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.status = Status::Cancelled;
            j.detail = "Cancelled · partial files kept for retry".into();
        }
        self.save();
    }
    fn retry(&mut self, id: u64) {
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == id)
        {
            return;
        }
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.status = Status::Queued;
            j.progress = 0.0;
            j.detail = "Waiting in queue".into();
        }
        self.tab = 0;
        self.save();
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some((_, flag)) = &self.active {
            flag.store(true, Ordering::Relaxed);
        }
        // Wait for the worker to kill its process group before the UI process exits.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn setup(s: &mut State, waker: rune::Waker) {
    s.tools.set_waker(waker);
    s.tools.request((), |_| {
        (
            engine::tool_command("yt-dlp")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success()),
            engine::tool_command("ffmpeg")
                .arg("-version")
                .output()
                .is_ok_and(|o| o.status.success()),
        )
    });
}
fn poll(s: &mut State, _: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = false;
    if let Some(tools) = s.tools.take() {
        s.ready = Some(tools);
        changed = true;
    }
    while let Ok(event) = s.rx.try_recv() {
        match event {
            Event::Line(id, value) => {
                if let Some(j) = s
                    .jobs
                    .iter_mut()
                    .find(|j| j.id == id && j.status == Status::Running)
                {
                    engine::apply_line(j, &value);
                }
            }
            Event::End(id, result) => {
                if let Some(j) = s
                    .jobs
                    .iter_mut()
                    .find(|j| j.id == id && j.status != Status::Cancelled)
                {
                    match result {
                        Ok(()) => {
                            j.status = Status::Complete;
                            j.progress = 1.0;
                            j.detail = "Saved to your library".into();
                        }
                        Err(e) => {
                            j.status = Status::Failed;
                            j.detail = e;
                        }
                    }
                }
                s.active = None;
                if let Some(worker) = s.worker.take() {
                    let _ = worker.join();
                }
                s.save();
            }
        }
        changed = true;
    }
    if s.active.is_none() && s.ready == Some((true, true)) {
        // Oldest queued item runs first, one process at a time.
        if let Some(j) = s.jobs.iter_mut().rev().find(|j| j.status == Status::Queued) {
            j.status = Status::Running;
            j.detail = "Getting media information…".into();
            let cancel = Arc::new(AtomicBool::new(false));
            s.active = Some((j.id, cancel.clone()));
            s.worker = Some(engine::spawn(j.clone(), s.tx.clone(), cancel));
            changed = true;
        }
    }
    if s.active.is_some() {
        tx.wake_after(Duration::from_millis(100));
    }
    changed
}
fn open_folder(path: PathBuf) {
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(target_os = "windows")]
        let mut cmd = Command::new("explorer");
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let mut cmd = Command::new("xdg-open");
        let _ = cmd.arg(path).status();
    });
}
fn navigation(s: &State) -> El {
    let mut nav = col()
        .w(196.0)
        .h_full()
        .padding(20.0)
        .gap(8.0)
        .fill(c("#111914"));
    nav = nav.child(
        row()
            .h(64.0)
            .gap(10.0)
            .center_y()
            .child(
                el().size(32., 32.)
                    .radius(10.)
                    .fill(mint())
                    .center()
                    .child(glyph("download", 20., c("#13271b"))),
            )
            .child(label("emul", 24., 650., ink())),
    );
    nav = nav.child(label("YOUR WORKSPACE", 10., 600., muted()).pt(25.).pb(10.));
    for (tab, name, icon) in [(0, "Downloads", "download"), (1, "Completed", "check")] {
        let selected = s.tab == tab && !s.settings;
        let count = s
            .jobs
            .iter()
            .filter(|j| {
                if tab == 1 {
                    j.status == Status::Complete
                } else {
                    j.status != Status::Complete
                }
            })
            .count();
        nav = nav.child(
            row()
                .key(format!("nav-{tab}"))
                .role("button")
                .label(name)
                .focusable(true)
                .h(42.)
                .px(10.)
                .gap(10.)
                .center_y()
                .radius(7.)
                .fill(if selected {
                    c("#26362b")
                } else {
                    Color::TRANSPARENT
                })
                .hover(c("#ffffff").alpha(0.04))
                .child(glyph(icon, 17., if selected { mint() } else { muted() }))
                .child(label(
                    name,
                    12.,
                    500.,
                    if selected { ink() } else { muted() },
                ))
                .child(space())
                .child(label(count.to_string(), 11., 500., muted()))
                .on_press(move |s: &mut State| {
                    s.tab = tab;
                    s.settings = false;
                }),
        );
    }
    nav.child(space())
        .child(
            col()
                .gap(7.)
                .padding(12.)
                .radius(9.)
                .border(1., line())
                .child(label("Made to keep.", 12., 600., ink()))
                .child(label("Your media. Your device.", 10., 400., muted())),
        )
        .child(
            button("settings", "Settings", false)
                .on_press(|s: &mut State| s.settings = !s.settings),
        )
        .child(label("RUNE + YT-DLP", 9., 500., muted()).pt(12.))
}
fn composer(s: &State) -> El {
    let field = text_input(&s.input, TextStyle::new(14., 400.))
        .key("media-url")
        .label("Media URL")
        .single_line()
        .grow(1.)
        .h(22.)
        .min_w(0.)
        .placeholder("Paste a video or audio link…", muted())
        .color(ink())
        .caret(mint(), mint().alpha(0.2))
        .on_edit(|s: &mut State| &mut s.input)
        .on_submit(|s: &mut State, _| s.enqueue());
    let paste = button("paste", "Paste link", false).on_press_tx(|_: &mut State, tx| {
        tx.read_clipboard(|s: &mut State, content, _| {
            s.input = TextEdit::new(content.text());
            s.error.clear();
        })
    });
    let input = row()
        .gap(12.)
        .px(16.)
        .center_y()
        .h(68.)
        .radius(10.)
        .fill(c("#111915"))
        .border(1., c("#3c5143"))
        .focus_within_ring(1., 0., mint())
        .child(glyph("link", 20., muted()))
        .child(field)
        .child(paste);
    let mut formats = row().gap(6.).padding(4.).radius(9.).fill(c("#111915"));
    for format in [Format::Video, Format::Audio, Format::Subtitles] {
        formats = formats.child(
            row()
                .key(format!("format-{}", format.label()))
                .role("button")
                .label(format.label())
                .focusable(true)
                .h(35.)
                .px(14.)
                .gap(8.)
                .center_y()
                .radius(6.)
                .fill(if s.format == format {
                    c("#34433a")
                } else {
                    Color::TRANSPARENT
                })
                .hover(c("#ffffff").alpha(0.05))
                .child(glyph(
                    match format {
                        Format::Video => "video",
                        Format::Audio => "audio",
                        Format::Subtitles => "subs",
                    },
                    15.,
                    if s.format == format { ink() } else { muted() },
                ))
                .child(label(
                    format.label(),
                    12.,
                    500.,
                    if s.format == format { ink() } else { muted() },
                ))
                .on_press(move |s: &mut State| s.format = format),
        );
    }
    let quality_label = match s.format {
        Format::Audio => "MP3 · best quality",
        Format::Subtitles => "English · SRT",
        Format::Video => match s.quality {
            720 => "720p HD",
            1080 => "1080p HD",
            2160 => "4K UHD",
            _ => "Best available",
        },
    };
    let quality = button("quality", format!("{quality_label} ↕"), false)
        .disabled(s.format != Format::Video)
        .help("Cycle quality: 720p, 1080p, 4K, best available")
        .on_press(|s: &mut State| {
            s.quality = match s.quality {
                720 => 1080,
                1080 => 2160,
                2160 => 0,
                _ => 720,
            }
        });
    let mut panel = col()
        .w_full()
        .padding(24.)
        .gap(20.)
        .radius(14.)
        .fill(c("#1b2820"))
        .border(1., c("#34463a"))
        .child(
            row()
                .justify_between()
                .center_y()
                .child(label("A link is all you need.", 18., 600., ink()))
                .child(label("VIDEO / AUDIO / SUBTITLES", 10., 500., muted())),
        )
        .child(input)
        .child(
            row()
                .gap(12.)
                .center_y()
                .child(formats)
                .child(quality)
                .child(space())
                .child(
                    button("download", "Download  ↓", true).on_press(|s: &mut State| s.enqueue()),
                ),
        )
        .child(
            row()
                .gap(7.)
                .center_y()
                .child(glyph("folder", 13., muted()))
                .child(
                    label(s.directory.to_string_lossy(), 11., 400., muted())
                        .truncate()
                        .min_w(0.)
                        .grow(1.),
                )
                .child(label("One link per download", 11., 400., muted())),
        );
    if !s.error.is_empty() {
        panel = panel.child(label(&s.error, 12., 400., c("#ffb9a6")).wrap());
    }
    panel
}
fn job_row(j: &Job) -> El {
    let id = j.id;
    let tint = match j.format {
        Format::Video => c("#a9cabc"),
        Format::Audio => c("#d7c49d"),
        Format::Subtitles => c("#a4c9d7"),
    };
    let status_color = match j.status {
        Status::Complete => mint(),
        Status::Failed => c("#ffb9a6"),
        _ => muted(),
    };
    let mut info = col()
        .grow(1.)
        .min_w(0.)
        .gap(7.)
        .child(label(&j.title, 14., 550., ink()).truncate().w_full())
        .child(
            label(
                format!(
                    "{}  ·  {}",
                    url::Url::parse(&j.url)
                        .ok()
                        .and_then(|u| u.host_str().map(String::from))
                        .unwrap_or_default(),
                    j.format.label()
                ),
                11.,
                400.,
                muted(),
            )
            .truncate(),
        );
    if j.status == Status::Running {
        info = info.child(
            el().w_full().h(3.).radius(2.).fill(line()).child(
                el().w_frac(j.progress.max(0.015))
                    .h(3.)
                    .radius(2.)
                    .fill(mint()),
            ),
        );
    }
    let status = match j.status {
        Status::Queued => "Queued".into(),
        Status::Running => format!("{:.0}%", j.progress * 100.),
        Status::Complete => "Saved".into(),
        Status::Cancelled => "Cancelled".into(),
        Status::Failed => "Needs attention".into(),
    };
    let status_text = if j.status == Status::Running {
        j.detail.clone()
    } else if j.status == Status::Complete {
        "Ready when you are".into()
    } else {
        j.detail.clone()
    };
    let action = if matches!(j.status, Status::Running | Status::Queued) {
        button(format!("cancel-{id}"), "Cancel", false).on_press(move |s: &mut State| s.cancel(id))
    } else if j.status == Status::Complete {
        let dir = j.directory.clone();
        button(format!("open-{id}"), "Open folder", false)
            .on_press(move |_: &mut State| open_folder(dir.clone()))
    } else {
        button(format!("retry-{id}"), "Retry", false).on_press(move |s: &mut State| s.retry(id))
    };
    let mut card = col()
        .key(format!("job-{id}"))
        .w_full()
        .gap(14.)
        .py(20.)
        .child(
            row()
                .center_y()
                .gap(18.)
                .child(
                    el().size(62., 56.)
                        .radius(9.)
                        .fill(tint.alpha(0.1))
                        .border(1., tint.alpha(0.13))
                        .center()
                        .child(glyph(
                            match j.format {
                                Format::Video => "video",
                                Format::Audio => "audio",
                                Format::Subtitles => "subs",
                            },
                            24.,
                            tint,
                        )),
                )
                .child(info)
                .child(
                    col()
                        .w(155.)
                        .gap(6.)
                        .child(label(status, 12., 550., status_color))
                        .child(
                            label(status_text, 10., 400., muted())
                                .truncate()
                                .help(j.detail.clone()),
                        ),
                )
                .child(action),
        );
    if j.status == Status::Failed {
        card = card.child(label(&j.detail, 12., 400., c("#ffb9a6")).wrap().w_full());
    }
    card.child(rule())
}
fn queue(s: &State) -> El {
    let shown: Vec<_> = s
        .jobs
        .iter()
        .filter(|j| {
            if s.tab == 1 {
                j.status == Status::Complete
            } else {
                j.status != Status::Complete
            }
        })
        .collect();
    let mut content = col().w_full().min_h(0.).grow(1.).gap(8.).child(
        row()
            .h(44.)
            .center_y()
            .gap(10.)
            .child(label(
                if s.tab == 1 {
                    "Completed"
                } else {
                    "Download queue"
                },
                17.,
                600.,
                ink(),
            ))
            .child(label(format!("{:02}", shown.len()), 11., 500., muted()))
            .child(space())
            .child(
                button("clear-history", "Clear finished", false).on_press(|s: &mut State| {
                    s.jobs
                        .retain(|j| matches!(j.status, Status::Running | Status::Queued));
                    s.save();
                }),
            ),
    );
    if shown.is_empty() {
        content = content.child(
            col()
                .grow(1.)
                .min_h(220.)
                .w_full()
                .center()
                .gap(14.)
                .child(
                    el().size(76., 76.)
                        .radius(24.)
                        .fill(c("#1d2b22"))
                        .border(1., c("#314438"))
                        .center()
                        .child(glyph(
                            if s.tab == 1 { "check" } else { "download" },
                            32.,
                            mint(),
                        )),
                )
                .child(label(
                    if s.tab == 1 {
                        "Your collection starts here."
                    } else {
                        "Good things are worth keeping."
                    },
                    20.,
                    550.,
                    ink(),
                ))
                .child(label(
                    if s.tab == 1 {
                        "Finished downloads will appear here."
                    } else {
                        "Paste a link above. We’ll take it from there."
                    },
                    13.,
                    400.,
                    muted(),
                )),
        );
    } else {
        content = content.child(
            col()
                .key("queue-scroll")
                .grow(1.)
                .min_h(0.)
                .w_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(col().w_full().children(shown.into_iter().map(job_row))),
        );
    }
    content
}
fn settings(s: &State) -> El {
    col().gap(24.).w_full().child(label("Download settings",28.,600.,ink())).child(label("Files stay on your device. Choose where they land.",14.,400.,muted()))
        .child(col().padding(24.).gap(16.).radius(12.).fill(c("#1b2820")).border(1.,line()).child(label("Download folder",14.,550.,ink()))
            .child(text_input(&s.folder,TextStyle::new(14.,400.)).key("folder-path").label("Absolute download folder").single_line().w_full().h(48.).px(14.).py(14.).radius(7.).fill(c("#111915")).border(1.,line()).color(ink()).on_edit(|s: &mut State| &mut s.folder))
            .child(label("Use an absolute path. Changes apply to new downloads.",12.,400.,muted()))
            .child(button("save-settings","Save folder",true).self_start().on_press(|s: &mut State| {
                if s.demo {s.settings=false;return;}
                let p=PathBuf::from(s.folder.text.trim());
                if !p.is_absolute() {s.error="Enter an absolute folder path.".into();return;}
                let result=std::fs::create_dir_all(&p).and_then(|_|std::fs::create_dir_all(engine::history_path().parent().unwrap())).and_then(|_|std::fs::write(engine::history_path().with_file_name("folder.txt"),p.to_string_lossy().as_bytes()));
                match result {Ok(())=>{s.directory=p;s.settings=false;s.error.clear();},Err(e)=>s.error=e.to_string()}
            })))
        .child(label("Engine dependencies",17.,600.,ink())).child(label(format!("yt-dlp: {}     FFmpeg: {}", if s.ready.is_some_and(|t|t.0){"Installed"}else{"Missing / checking"},if s.ready.is_some_and(|t|t.1){"Installed"}else{"Missing / checking"}),14.,400.,muted()))
        .child(label("Video keeps the source codecs in MKV. Audio is saved as MP3. Subtitles use English SRT when available. Each download saves one video; playlists are disabled.",13.,400.,muted()).wrap())
        .child(label(&s.error,13.,400.,c("#ffb9a6")).wrap())
}
fn view(s: &State, _: &mut Cx<State>) -> El {
    let complete = s
        .jobs
        .iter()
        .filter(|j| j.status == Status::Complete)
        .count();
    let header = row()
        .h(56.)
        .px(32.)
        .center_y()
        .gap(8.)
        .border_sides([0., 0., 1., 0.], line())
        .window_drag()
        .child(label("LIBRARY", 10., 600., muted()))
        .child(label("/", 12., 400., line()))
        .child(label(
            if s.settings { "Settings" } else { "Downloads" },
            12.,
            500.,
            ink(),
        ))
        .child(space())
        .child(
            el().size(6., 6.)
                .radius(3.)
                .fill(if s.ready == Some((true, true)) {
                    mint()
                } else {
                    c("#d7c49d")
                }),
        )
        .child(label(
            if s.ready == Some((true, true)) {
                "Engine ready"
            } else {
                "Check dependencies"
            },
            10.,
            500.,
            muted(),
        ));
    let mut body = col().grow(1.).min_h(0.).w_full().padding(32.).gap(26.);
    if s.settings {
        body = body.child(settings(s)).child(space());
    } else {
        body = body
            .child(
                row().center_y().justify_between().child(
                    col()
                        .gap(8.)
                        .child(label("Keep a little of the internet.", 30., 600., ink()))
                        .child(label(
                            "Save videos, audio, and subtitles. All in one place.",
                            13.,
                            400.,
                            muted(),
                        )),
                ),
            )
            .child(composer(s))
            .child(queue(s));
    }
    row()
        .key("app")
        .w_full()
        .h_full()
        .fill(c("#151e18"))
        .child(navigation(s))
        .child(
            col()
                .grow(1.)
                .min_w(0.)
                .h_full()
                .child(header)
                .child(body)
                .child(
                    row()
                        .h(38.)
                        .px(32.)
                        .gap(8.)
                        .center_y()
                        .border_sides([1., 0., 0., 0.], line())
                        .child(glyph("folder", 13., muted()))
                        .child(label(format!("{complete} files saved"), 10., 400., muted()))
                        .child(space())
                        .child(label("Local files. No account needed.", 10., 400., muted())),
                ),
        )
}
fn showcase(s: &mut State) {
    s.demo = true;
    s.folder = TextEdit::new("/Users/you/Downloads/emul");
    s.directory = PathBuf::from("/Users/you/Downloads/emul");
    s.ready = Some((true, true));
    s.jobs.clear();
    for (id, title, url, format, status, progress, detail) in [
        (
            1,
            "A quiet morning in the mountains",
            "https://vimeo.com/preview",
            Format::Video,
            Status::Running,
            0.64,
            "8.4 MB/s  ·  12s left",
        ),
        (
            2,
            "The art of paying attention",
            "https://youtube.com/watch?v=preview",
            Format::Audio,
            Status::Queued,
            0.,
            "Waiting in queue",
        ),
        (
            3,
            "Notes from the creative process",
            "https://youtube.com/watch?v=preview2",
            Format::Subtitles,
            Status::Queued,
            0.,
            "Waiting in queue",
        ),
    ] {
        s.jobs.push(Job {
            id,
            url: url.into(),
            title: title.into(),
            format,
            quality: 1080,
            directory: PathBuf::from(&s.folder.text),
            status,
            progress,
            detail: detail.into(),
            file: None,
        });
    }
}
fn main() {
    if std::env::args().any(|arg| arg == "--check-tools") {
        for (tool, flag) in [
            ("yt-dlp", "--version"),
            ("ffmpeg", "-version"),
            ("ffprobe", "-version"),
            ("deno", "--version"),
        ] {
            match engine::tool_command(tool).arg(flag).output() {
                Ok(output) if output.status.success() => {
                    println!(
                        "{tool}: {}",
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .next()
                            .unwrap_or("OK")
                    );
                }
                result => {
                    eprintln!("{tool} check failed: {result:?}");
                    std::process::exit(1);
                }
            }
        }
        return;
    }
    rune::tooling::set_app_id("org.emul.downloader");
    rune::run(AppSpec {
        min_size: Some((980., 720.)),
        setup: Some(setup),
        poll: Some(poll),
        scenes: &[("showcase", showcase)],
        ..AppSpec::new("emul — Media downloader", (1180., 820.), State::new, view)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};
    fn runtime() -> Runtime<State> {
        let mut s = State::new();
        s.demo = true;
        s.jobs.clear();
        s.ready = Some((true, true));
        let mut rt = Runtime::new(s, view, (1180., 820.), Env::new(true, 1.));
        rt.frame(0.);
        rt
    }
    fn tap(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("Missing control {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.);
    }
    #[test]
    fn format_selection_and_download_action_create_a_real_queued_job() {
        let mut rt = runtime();
        tap(&mut rt, "format-Audio");
        assert_eq!(rt.state.format, Format::Audio);
        rt.state.input = TextEdit::new("https://example.com/video");
        rt.frame(0.);
        tap(&mut rt, "download");
        assert_eq!(rt.state.jobs.len(), 1);
        assert_eq!(rt.state.jobs[0].format, Format::Audio);
        assert_eq!(rt.state.jobs[0].status, Status::Queued);
        assert!(rt.state.input.text.is_empty());
    }
    #[test]
    fn malformed_links_never_enter_the_queue() {
        let mut rt = runtime();
        rt.state.input = TextEdit::new("file:///etc/passwd");
        tap(&mut rt, "download");
        assert!(rt.state.jobs.is_empty());
        assert!(!rt.state.error.is_empty());
    }
    #[test]
    fn queue_cancel_retry_and_history_navigation_work() {
        let mut rt = runtime();
        rt.state.input = TextEdit::new("https://example.com/video");
        tap(&mut rt, "download");
        tap(&mut rt, "cancel-1");
        assert_eq!(rt.state.jobs[0].status, Status::Cancelled);
        tap(&mut rt, "retry-1");
        assert_eq!(rt.state.jobs[0].status, Status::Queued);
        tap(&mut rt, "nav-1");
        assert_eq!(rt.state.tab, 1);
        tap(&mut rt, "settings");
        assert!(rt.state.settings);
    }
}
