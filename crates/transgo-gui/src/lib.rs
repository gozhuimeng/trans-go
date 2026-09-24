//! transgo 图形界面。
//!
//! 独立的输入翻译窗口，封装 [`transgo_core`] 的服务：不做守护进程、不监听全局按键
//! （Wayland 也不允许）、不监听剪贴板。`--clip` 只在启动时读一次剪贴板预填源文本，
//! 然后立即翻一次。
//!
//! 翻译在后台线程里跑（内部是 tokio 当前线程运行时），界面只在每帧轮询结果，
//! 不阻塞事件循环。

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use transgo_core::config::Config;
use transgo_core::engine::{self, Engine};
use transgo_core::{Lang, Request, Translation};

/// 实例之间递的消息。协议是纯文本：`show`，或 `translate\n<文本>`
enum AppMsg {
    Show,
    Translate(String),
}

fn encode(msg: &AppMsg) -> String {
    match msg {
        AppMsg::Show => "show".to_string(),
        AppMsg::Translate(t) => format!("translate\n{t}"),
    }
}

fn decode(raw: &str) -> Option<AppMsg> {
    match raw {
        "show" => Some(AppMsg::Show),
        _ => raw
            .strip_prefix("translate\n")
            .map(|t| AppMsg::Translate(t.to_string())),
    }
}

/// 实例间通信用的 socket，落在 XDG_RUNTIME_DIR（用户私有 tmpfs）
fn socket_path() -> std::path::PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("transgo-gui.sock")
}

/// 单例闸门：已有实例在跑就把消息递过去并返回 true，调用方直接退出。
/// 连不上就把残留的 socket 文件清掉，换新进程接管。
fn notify_existing(msg: &AppMsg) -> bool {
    let path = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&path) {
        return stream.write_all(encode(msg).as_bytes()).is_ok();
    }
    let _ = std::fs::remove_file(&path);
    false
}

fn bind_listener() -> Result<UnixListener, String> {
    let path = socket_path();
    UnixListener::bind(&path).map_err(|e| format!("监听 {} 失败: {e}", path.display()))
}

/// 后台线程：收第二个实例递来的消息，转给界面线程并触发重绘
fn listen_loop(listener: UnixListener, tx: Sender<AppMsg>, ctx: &egui::Context) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let mut raw = String::new();
        if stream.read_to_string(&mut raw).is_err() {
            continue;
        }
        if let Some(msg) = decode(&raw) {
            let _ = tx.send(msg);
            ctx.request_repaint();
        }
    }
}

/// 界面入口。窗口关闭时返回。
pub fn run(clip: bool) -> Result<(), String> {
    let preset = if clip { clipboard_text() } else { None };

    // 单例：已有窗口就不开第二扇，把消息递过去（--clip 会把剪贴板文本送进去直翻）
    let msg = match &preset {
        Some(t) if !t.trim().is_empty() => AppMsg::Translate(t.clone()),
        _ => AppMsg::Show,
    };
    if notify_existing(&msg) {
        return Ok(());
    }
    let listener = bind_listener()?;

    let cfg = Config::load().map_err(|e| e.to_string())?;
    let engines = engine::build_all(&cfg);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 460.0])
            .with_min_inner_size([420.0, 280.0]),
        ..Default::default()
    };

    let result = eframe::run_native(
        "transgo",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            setup_style(&cc.egui_ctx);
            let (msg_tx, msg_rx) = mpsc::channel();
            {
                let ctx = cc.egui_ctx.clone();
                std::thread::spawn(move || listen_loop(listener, msg_tx, &ctx));
            }
            Box::new(App::new(engines, preset, msg_rx))
        }),
    )
    .map_err(|e| e.to_string());

    // 窗口关了把门牌摘掉；就算漏摘，下次启动的 notify_existing 也会清理残留
    let _ = std::fs::remove_file(socket_path());
    result
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
    instruction: String,
    /// 启动后把焦点落到源输入框，省一次点击
    focus_source: bool,
    status: String,
    error: Option<String>,
    busy: bool,
    copied_at: Option<Instant>,
    clip_loaded: bool,
    job_tx: Sender<Job>,
    done_rx: Receiver<Done>,
    /// 从第二个实例递过来的消息
    msg_rx: Receiver<AppMsg>,
}

impl App {
    fn new(
        engines: Vec<Arc<dyn Engine>>,
        preset: Option<String>,
        msg_rx: Receiver<AppMsg>,
    ) -> Self {
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
            instruction: String::new(),
            focus_source: true,
            status: String::new(),
            error: None,
            busy: false,
            copied_at: None,
            clip_loaded: preset.is_some(),
            job_tx,
            done_rx,
            msg_rx,
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
        let req = Request::new(self.source.clone(), Some(self.from), Some(self.to))
            .with_instruction(Some(self.instruction.clone()));
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

        // 第二个实例递来的消息：聚焦窗口，或换上新文本直接翻
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                AppMsg::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                AppMsg::Translate(text) => {
                    self.source = text;
                    self.start_translate();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }

        // ESC 一键关窗：不挑桌面环境，不用去点右上角
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

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

        egui::CentralPanel::default()
            .frame(egui::Frame {
                // 必须显式给不透明的填充：Frame::default() 是透明的，会透出桌面壁纸
                fill: egui::Color32::WHITE,
                inner_margin: egui::Margin::same(14.0),
                ..egui::Frame::default()
            })
            .show(ctx, |ui| {
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
                    let btn = egui::Button::new(egui::RichText::new("翻译").color(egui::Color32::WHITE))
                        .fill(ACCENT);
                    if ui.add_enabled(!self.busy, btn).clicked() {
                        self.start_translate();
                    }
                    if self.busy {
                        ui.spinner();
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            ui.collapsing("翻译指令（可选）", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.instruction)
                        .desired_width(f32::INFINITY)
                        .hint_text("例如：采用意译、用学术风格。仅大模型翻译生效，留空为默认风格"),
                );
            });
            ui.add_space(2.0);

            // 字符数给个直观读数：两家百度接口都按源字符计费
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("原文").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} 字符", self.source.chars().count()))
                            .color(META),
                    );
                });
            });

            let source_edit = ui.add(
                egui::TextEdit::multiline(&mut self.source)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .hint_text("输入要翻译的文本，回车翻译，Shift + 回车换行"),
            );
            if self.focus_source {
                source_edit.request_focus();
                self.focus_source = false;
            }

            if enter {
                self.start_translate();
            }

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("译文").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).color(META));
                });
            });

            ui.add(
                egui::TextEdit::multiline(&mut self.output)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .hint_text("译文会显示在这里"),
            );

            ui.horizontal(|ui| {
                if ui.button("复制译文").clicked() {
                    self.copy_output();
                }
                if let Some(at) = self.copied_at {
                    if at.elapsed() < Duration::from_secs(2) {
                        ui.label("已复制");
                        ctx.request_repaint_after(Duration::from_millis(500));
                    } else {
                        self.copied_at = None;
                    }
                }
            });

            if let Some(err) = &self.error {
                ui.add_space(2.0);
                ui.colored_label(
                    egui::Color32::from_rgb(200, 60, 60),
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

// --------------------------------------------------------------------- 外观

/// 点缀色：翻译主按钮
const ACCENT: egui::Color32 = egui::Color32::from_rgb(52, 112, 214);
/// 元信息（字符数、状态行）用的中间灰，比 weak 更清楚一点
const META: egui::Color32 = egui::Color32::from_gray(110);

/// 浅色主题 + 大字号 + 明确的控件边框。egui 默认深底灰字、字号偏小、
/// 文本框边框几乎不可见，这里统一调掉。
fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    // 纯白底，不透桌面
    style.visuals.panel_fill = egui::Color32::WHITE;

    // 下拉框、按钮统一最小高度，避免顶栏参差
    style.spacing.interact_size.y = 36.0;

    // 文本框、下拉框的默认边框太淡，加深一点，顺带统一圆角
    let border = egui::Stroke::new(1.0_f32, egui::Color32::from_gray(150));
    let rounding = egui::Rounding::same(4.0);
    for w in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        w.bg_stroke = border;
        w.rounding = rounding;
    }

    // 控件之间松一点
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);

    // 字号整体加大一档
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            egui::FontId::new(24.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            egui::FontId::new(18.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            egui::FontId::new(18.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Small,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            egui::FontId::new(17.0, egui::FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style(style);
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
            .insert("cjk".to_string(), egui::FontData::from_owned(bytes));
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

#[cfg(test)]
mod tests {
    use super::{decode, encode, AppMsg};

    #[test]
    fn relay_message_roundtrip() {
        assert!(matches!(decode(&encode(&AppMsg::Show)), Some(AppMsg::Show)));
        match decode(&encode(&AppMsg::Translate("知识就是力量".into()))) {
            Some(AppMsg::Translate(t)) => assert_eq!(t, "知识就是力量"),
            _ => panic!("translate 消息应能往返"),
        }
        // 多行文本不串行、脏消息不误认
        let multi = "第一行\n第二行";
        match decode(&encode(&AppMsg::Translate(multi.into()))) {
            Some(AppMsg::Translate(t)) => assert_eq!(t, multi),
            _ => panic!("多行文本应能往返"),
        }
        assert!(decode("garbage").is_none());
    }
}
