//! transgo 图形界面。
//!
//! 独立的输入翻译窗口，封装 [`transgo_core`] 的服务：不做守护进程、不监听全局按键
//! （Wayland 也不允许）、不监听剪贴板。`--clip` 只在启动时读一次剪贴板预填源文本，
//! 然后立即翻一次。
//!
//! 翻译在后台线程里跑（内部是 tokio 当前线程运行时），界面只在每帧轮询结果，
//! 不阻塞事件循环。

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use transgo_core::config::Config;
use transgo_core::engine::{self, Engine};
use transgo_core::{Lang, Request, Translation};

/// 界面入口。窗口关闭时返回。
pub fn run(clip: bool) -> Result<(), String> {
    let preset = if clip { clipboard_text() } else { None };
    let cfg = Config::load().map_err(|e| e.to_string())?;
    let engines = engine::build_all(&cfg);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 460.0])
            .with_min_inner_size([420.0, 280.0]),
        ..Default::default()
    };

    eframe::run_native(
        "transgo",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Box::new(App::new(engines, preset))
        }),
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 后台翻译线程

struct Job {
    engine: usize,
    req: Request,
}

struct Done {
    result: Result<Translation, String>,
    ms: f64,
}

/// 起一个后台线程逐个执行翻译任务。引擎的 `translate` 是 async 的，
/// 这里用一个当前线程运行时驱动，GUI 线程只收发消息。
fn spawn_worker(engines: Vec<Arc<dyn Engine>>) -> (Sender<Job>, Receiver<Done>) {
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (done_tx, done_rx) = mpsc::channel::<Done>();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        while let Ok(job) = job_rx.recv() {
            let Some(eng) = engines.get(job.engine) else {
                continue;
            };
            let t0 = Instant::now();
            let result = rt.block_on(eng.translate(&job.req));
            let done = Done {
                result: result.map_err(|e| e.to_string()),
                ms: t0.elapsed().as_secs_f64() * 1000.0,
            };
            if done_tx.send(done).is_err() {
                return;
            }
        }
    });
    (job_tx, done_rx)
}

// ---------------------------------------------------------------------- 状态

struct App {
    engines: Vec<Arc<dyn Engine>>,
    engine_idx: usize,
    from: Lang,
    to: Lang,
    source: String,
    output: String,
    status: String,
    error: Option<String>,
    busy: bool,
    copied_at: Option<Instant>,
    clip_loaded: bool,
    job_tx: Sender<Job>,
    done_rx: Receiver<Done>,
}

impl App {
    fn new(engines: Vec<Arc<dyn Engine>>, preset: Option<String>) -> Self {
        let configured: Vec<usize> = engines
            .iter()
            .enumerate()
            .filter(|(_, e)| e.configured())
            .map(|(i, _)| i)
            .collect();
        let engine_idx = configured.first().copied().unwrap_or(0);
        let (job_tx, done_rx) = spawn_worker(engines.clone());
        let mut app = App {
            engines,
            engine_idx,
            from: Lang::Auto,
            to: Lang::Auto,
            source: preset.clone().unwrap_or_default(),
            output: String::new(),
            status: String::new(),
            error: None,
            busy: false,
            copied_at: None,
            clip_loaded: preset.is_some(),
            job_tx,
            done_rx,
        };
        if app.clip_loaded {
            app.start_translate();
        }
        app
    }

    fn start_translate(&mut self) {
        if self.busy || self.source.trim().is_empty() {
            return;
        }
        let req = Request::new(self.source.clone(), Some(self.from), Some(self.to));
        let Some(eng) = self.engines.get(self.engine_idx) else {
            self.error = Some("没有可用引擎，先用 transgo config 填入 API Key".into());
            return;
        };
        if !eng.supports(&req) {
            self.error = Some(format!(
                "{} 不支持 {} → {} 这个方向",
                eng.name(),
                req.from.map(|l| l.name_zh()).unwrap_or("自动"),
                req.to.name_zh()
            ));
            return;
        }
        if self
            .job_tx
            .send(Job {
                engine: self.engine_idx,
                req,
            })
            .is_err()
        {
            self.error = Some("后台翻译线程已退出".into());
            return;
        }
        self.busy = true;
        self.error = None;
        self.status = "翻译中…".into();
    }

    fn poll_done(&mut self) {
        match self.done_rx.try_recv() {
            Ok(done) => {
                self.busy = false;
                match done.result {
                    Ok(t) => {
                        self.output = t.text;
                        self.status = format!(
                            "[{}] {} → {} · {:.0} ms",
                            t.engine,
                            t.from.name_zh(),
                            t.to.name_zh(),
                            done.ms
                        );
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.status.clear();
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.busy = false,
        }
    }

    fn copy_output(&mut self) {
        if self.output.is_empty() {
            return;
        }
        if set_clipboard(&self.output) {
            self.copied_at = Some(Instant::now());
        } else {
            self.error = Some("写入剪贴板失败".into());
        }
    }
}

// --------------------------------------------------------------------- 绘制

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_done();

        // 回车触发翻译：吃掉未加 Shift 的 Enter（连同它产生的换行），Shift + Enter 留给换行
        let mut enter = false;
        ctx.input_mut(|i| {
            let hit = i.events.iter().any(|e| {
                matches!(e,
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. }
                        if !modifiers.shift)
            });
            if hit {
                enter = true;
                i.events.retain(|e| match e {
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        modifiers,
                        ..
                    } => modifiers.shift,
                    egui::Event::Text(t) => t != "\n" && t != "\r",
                    _ => true,
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("engine")
                    .selected_text(
                        self.engines
                            .get(self.engine_idx)
                            .map(|e| e.name())
                            .unwrap_or("无可用引擎"),
                    )
                    .show_ui(ui, |ui| {
                        for (i, e) in self.engines.iter().enumerate() {
                            if e.configured() {
                                ui.selectable_value(&mut self.engine_idx, i, e.name());
                            }
                        }
                    });

                lang_combo(ui, "从", &mut self.from, true);
                lang_combo(ui, "到", &mut self.to, false);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("翻译"))
                        .clicked()
                    {
                        self.start_translate();
                    }
                    if self.busy {
                        ui.spinner();
                    }
                });
            });

            ui.separator();

            ui.add(
                egui::TextEdit::multiline(&mut self.source)
                    .desired_rows(8)
                    .hint_text("输入要翻译的文本，回车翻译，Shift + 回车换行"),
            );

            if enter {
                self.start_translate();
            }

            ui.horizontal(|ui| {
                if ui.button("复制译文").clicked() {
                    self.copy_output();
                }
                if let Some(at) = self.copied_at {
                    if at.elapsed() < Duration::from_secs(2) {
                        ui.label(egui::RichText::new("已复制").weak());
                        ctx.request_repaint_after(Duration::from_millis(500));
                    } else {
                        self.copied_at = None;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).weak());
                });
            });

            ui.add(
                egui::TextEdit::multiline(&mut self.output)
                    .desired_rows(8)
                    .hint_text("译文会显示在这里"),
            );

            if let Some(err) = &self.error {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 80, 80),
                    egui::RichText::new(err).small(),
                );
            }
        });
    }
}

fn lang_combo(ui: &mut egui::Ui, label: &str, sel: &mut Lang, is_from: bool) {
    let text = if *sel == Lang::Auto {
        if is_from {
            "自动检测".to_string()
        } else {
            "自动（中→英，其余→中）".to_string()
        }
    } else {
        sel.name_zh().to_string()
    };
    egui::ComboBox::from_id_source(format!("lang-{label}"))
        .selected_text(text)
        .show_ui(ui, |ui| {
            let auto = if is_from {
                "自动检测"
            } else {
                "自动（中→英，其余→中）"
            };
            ui.selectable_value(sel, Lang::Auto, auto);
            for l in Lang::ALL {
                ui.selectable_value(sel, *l, l.name_zh());
            }
        });
}

// ------------------------------------------------------------------ 字体与剪贴板

/// egui 自带字体没有汉字。从系统字体里找一款中文体挂上去，
/// 找不到就在状态栏提示（中文会显示成方框）。
fn install_fonts(ctx: &egui::Context) {
    const FAMILIES: &[&str] = &[
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Source Han Sans SC",
        "WenQuanYi Micro Hei",
        "WenQuanYi Zen Hei",
        "Droid Sans Fallback",
    ];

    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for family in FAMILIES {
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..Default::default()
        };
        let Some(id) = db.query(&query) else {
            continue;
        };
        let Some(info) = db.face(id) else {
            continue;
        };
        let fontdb::Source::File(path) = &info.source else {
            continue;
        };
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("cjk".to_string(), egui::FontData::from_owned(bytes).into());
        // 放在默认字体后面：拉丁字符用 egui 自带的，汉字落到这款
        for list in fonts.families.values_mut() {
            list.push("cjk".to_string());
        }
        ctx.set_fonts(fonts);
        return;
    }
}

fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok())
}

fn set_clipboard(s: &str) -> bool {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(s.to_string()))
        .is_ok()
}
