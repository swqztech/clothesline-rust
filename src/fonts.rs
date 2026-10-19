use eframe::egui::{FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

pub fn setup_cjk_fonts(ctx: &eframe::egui::Context) {
    let bytes = load_system_cjk_font();
    if bytes.is_empty() {
        return;
    }
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "cjk".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "cjk".to_owned());
    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Vec<u8> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let fonts_dir = PathBuf::from(windir).join("Fonts");
    for name in ["msyh.ttc", "msyhbd.ttc", "simhei.ttf"] {
        if let Ok(data) = std::fs::read(fonts_dir.join(name)) {
            return data;
        }
    }
    Vec::new()
}
