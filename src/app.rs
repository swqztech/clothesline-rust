use crate::file_index::{FileRecord, RopeState};
use crate::fonts;
use crate::settings::{self, Settings};
use crate::tray::TrayCommand;
use crate::watcher::{drain_records, WatcherEvent, WatcherHandle};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const PEG_COLORS: [egui::Color32; 6] = [
    egui::Color32::from_rgb(220, 70, 70),
    egui::Color32::from_rgb(240, 150, 40),
    egui::Color32::from_rgb(230, 200, 50),
    egui::Color32::from_rgb(70, 180, 90),
    egui::Color32::from_rgb(60, 130, 220),
    egui::Color32::from_rgb(150, 80, 200),
];

const COLLAPSED_H: f32 = 12.0;
const EXPANDED_H: f32 = 132.0;

pub struct ClotheslineApp {
    settings: Settings,
    rope: RopeState,
    records: Vec<FileRecord>,
    displayed: Vec<FileRecord>,
    expanded: bool,
    boot_expand_until: Instant,
    last_hover: Instant,
    watcher_rx: Receiver<WatcherEvent>,
    watcher: Option<WatcherHandle>,
    tray_rx: Receiver<TrayCommand>,
    dirty_settings: bool,
}

impl ClotheslineApp {
    pub fn new(tray_rx: Receiver<TrayCommand>) -> Self {
        let settings = settings::load();
        let always_visible = settings.always_visible;
        let rope = rope_from_settings(&settings);
        let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();
        let watcher = crate::watcher::spawn(
            settings.watch_folders.clone(),
            settings.manual_paths.clone(),
            watcher_tx,
        );

        Self {
            settings,
            rope,
            records: Vec::new(),
            displayed: Vec::new(),
            expanded: true,
            boot_expand_until: Instant::now() + Duration::from_secs(20),
            last_hover: Instant::now(),
            watcher_rx,
            watcher: Some(watcher),
            tray_rx,
            dirty_settings: false,
        }
    }

    fn sync_settings_from_rope(&mut self) {
        self.settings.dismissed = self
            .rope
            .dismissed
            .iter()
            .map(|(k, v)| (settings::path_to_key(k), v.clone()))
            .collect();
        self.settings.pinned = self
            .rope
            .pinned
            .iter()
            .map(|p| settings::path_to_key(p))
            .collect();
        self.dirty_settings = true;
    }

    fn apply_tray(&mut self, cmd: TrayCommand) {
        match cmd {
            TrayCommand::Quit => {}
            TrayCommand::ShowLine => {
                self.expanded = true;
                self.boot_expand_until = Instant::now() + Duration::from_secs(30);
                self.last_hover = Instant::now();
            }
            TrayCommand::ToggleAlwaysVisible => {
                self.settings.always_visible = !self.settings.always_visible;
                self.expanded = self.settings.always_visible;
                self.dirty_settings = true;
            }
            TrayCommand::Clear => {
                for f in self.displayed.clone() {
                    self.rope.dismiss(&f);
                }
                self.sync_settings_from_rope();
                self.refresh_displayed();
            }
            TrayCommand::RestoreDismissed => {
                self.rope.clear_dismissed();
                self.sync_settings_from_rope();
                self.refresh_displayed();
            }
            TrayCommand::RequestMountFolder => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    if !self.settings.manual_paths.contains(&folder) {
                        self.settings.manual_paths.push(folder);
                        self.dirty_settings = true;
                    }
                }
            }
        }
    }

    fn refresh_displayed(&mut self) {
        self.displayed = self.rope.select(&self.records, self.settings.max_items);
    }

    fn work_area() -> (f32, f32, f32) {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::RECT;
            use windows::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SPI_GETWORKAREA, SM_CXSCREEN, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
                SystemParametersInfoW,
            };
            let mut rect = RECT::default();
            unsafe {
                let _ = SystemParametersInfoW(
                    SPI_GETWORKAREA,
                    0,
                    Some(&mut rect as *mut _ as *mut _),
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                );
            }
            let w = (rect.right - rect.left) as f32;
            let x = rect.left as f32;
            let y = rect.top as f32;
            if w > 0.0 {
                return (x, y, w);
            }
            let w = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f32;
            (0.0, 0.0, w)
        }
        #[cfg(not(windows))]
        {
            (0.0, 0.0, 1280.0)
        }
    }

    fn layout_viewport(&mut self, ctx: &egui::Context) {
        let (x, y, w) = Self::work_area();
        let h = if self.expanded || self.settings.always_visible {
            EXPANDED_H
        } else {
            COLLAPSED_H
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
    }

    fn open_path(path: &PathBuf, is_dir: bool) {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            if is_dir {
                let _ = std::process::Command::new("explorer")
                    .arg(path)
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            } else {
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", path.to_string_lossy().as_ref()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            }
        }
        #[cfg(not(windows))]
        {
            let _ = path;
        }
    }

    fn reveal_in_explorer(path: &PathBuf) {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = std::process::Command::new("explorer")
                .args(["/select,", path.to_string_lossy().as_ref()])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
    }
}

fn rope_from_settings(s: &Settings) -> RopeState {
    let mut rope = RopeState::default();
    for (k, v) in &s.dismissed {
        rope.dismissed.insert(PathBuf::from(k), v.clone());
    }
    for p in &s.pinned {
        rope.pinned.insert(PathBuf::from(p));
    }
    rope
}

impl eframe::App for ClotheslineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rec) = drain_records(&self.watcher_rx) {
            self.records = rec;
        }
        for p in &self.settings.manual_paths {
            if let Some(r) = FileRecord::read(p) {
                if !self.records.iter().any(|x| x.path == r.path) {
                    self.records.push(r);
                }
            }
        }
        self.records.sort_by(|a, b| b.modified.cmp(&a.modified));
        self.refresh_displayed();
        while let Ok(cmd) = self.tray_rx.try_recv() {
            if matches!(cmd, TrayCommand::Quit) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.apply_tray(cmd);
        }
        if self.dirty_settings {
            settings::save(&self.settings);
            self.dirty_settings = false;
        }

        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let screen = ctx.input(|i| i.viewport().outer_rect);
        if let (Some(p), Some(r)) = (pointer, screen) {
            if r.contains(p) {
                self.last_hover = Instant::now();
                self.expanded = true;
            } else if !self.settings.always_visible && self.last_hover.elapsed() > Duration::from_millis(400) {
                self.expanded = false;
            }
        }

        if self.settings.always_visible || Instant::now() < self.boot_expand_until {
            self.expanded = true;
        }

        // 收起时仍检测鼠标是否在屏幕工作区顶缘，便于命中 8px 感应条
        if !self.expanded && !self.settings.always_visible {
            let (_, y, w) = Self::work_area();
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let outer = ctx.input(|i| i.viewport().outer_rect);
                if let Some(r) = outer {
                    let screen_y = r.min.y + pos.y;
                    if screen_y <= y + 12.0 && pos.x >= 0.0 && pos.x <= w {
                        self.expanded = true;
                        self.last_hover = Instant::now();
                    }
                }
            }
        }

        self.layout_viewport(ctx);
        ctx.request_repaint_after(Duration::from_millis(200));

        let bg = egui::Color32::from_rgba_premultiplied(28, 32, 38, 235);
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg).inner_margin(4.0))
            .show(ctx, |ui| {
                let (x, _y, w) = Self::work_area();
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, 6.0)),
                    |ui| {
                        ui.visuals_mut().widgets.inactive.bg_fill =
                            egui::Color32::from_rgb(90, 70, 50);
                        let _ = ui.allocate_response(egui::vec2(w, 4.0), egui::Sense::hover());
                    },
                );

                if !self.expanded && !self.settings.always_visible {
                    return;
                }

                let cards = self.displayed.clone();
                let zh = self.settings.locale_zh;
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for (idx, file) in cards.iter().enumerate() {
                        let peg = PEG_COLORS[idx % PEG_COLORS.len()];
                        let name = file.name();
                        let mut chars: String = name.chars().take(8).collect();
                        if name.chars().count() > 8 {
                            chars.push('…');
                        }
                        let label = chars;
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(8.0, 10.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 1.0, peg);
                            });
                            let card = egui::Frame::none()
                                .fill(egui::Color32::from_rgb(45, 50, 58))
                                .stroke(egui::Stroke::new(1.0, peg))
                                .inner_margin(6.0)
                                .rounding(4.0);
                            card.show(ui, |ui| {
                                let icon = if file.is_directory { "📁" } else { "📄" };
                                ui.label(icon);
                                let resp = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(label).size(11.0).color(egui::Color32::WHITE),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if resp.double_clicked() {
                                    Self::open_path(&file.path, file.is_directory);
                                }
                                resp.context_menu(|ui| {
                                    if ui.button(if zh { "打开" } else { "Open" }).clicked() {
                                        Self::open_path(&file.path, file.is_directory);
                                        ui.close_menu();
                                    }
                                    if ui.button(if zh { "在资源管理器中显示" } else { "Show in Explorer" }).clicked() {
                                        Self::reveal_in_explorer(&file.path);
                                        ui.close_menu();
                                    }
                                    if ui.button(if zh { "固定" } else { "Pin" }).clicked() {
                                        self.rope.pinned.insert(file.path.clone());
                                        self.sync_settings_from_rope();
                                        self.refresh_displayed();
                                        ui.close_menu();
                                    }
                                    if ui.button(if zh { "取下" } else { "Dismiss" }).clicked() {
                                        self.rope.dismiss(file);
                                        self.sync_settings_from_rope();
                                        self.refresh_displayed();
                                        ui.close_menu();
                                    }
                                });
                            });
                        });
                        ui.add_space(6.0);
                    }
                });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(w) = self.watcher.take() {
            w.stop();
        }
        settings::save(&self.settings);
    }
}

pub fn run(tray_rx: Receiver<TrayCommand>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_taskbar(false),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Clothesline",
        options,
        Box::new(|cc| {
            fonts::setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(ClotheslineApp::new(tray_rx)))
        }),
    )
}
