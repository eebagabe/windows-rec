//! GameRec - gravador de tela com audio para gameplays (Windows).
#![windows_subsystem = "windows"]

mod audio;
mod config;
mod controller;
#[macro_use]
mod log;
mod theme;
mod ui;
mod video;
mod win;
mod window;

mod clock {
    use std::sync::OnceLock;
    use std::time::Instant;

    static EPOCH: OnceLock<Instant> = OnceLock::new();

    /// Segundos desde o inicio do app; base comum para alinhar audio e video.
    pub fn now() -> f64 {
        EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64()
    }
}

use std::sync::Arc;

pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() {
    // usado pelo instalador para saber se o Windows deixa o exe rodar
    if std::env::args().any(|a| a == "--check") {
        return;
    }
    clock::now();
    std::panic::set_hook(Box::new(|info| {
        log!("PANIC: {info}");
        win::message_box("GameRec", &format!("Erro inesperado: {info}\n\nDetalhes em {}", log::path().display()));
    }));
    log!(
        "GameRec {} iniciado; ffmpeg: {} (existe: {})",
        env!("CARGO_PKG_VERSION"),
        video::ffmpeg_path().display(),
        video::ffmpeg_path().exists()
    );
    let icon = image::load_from_memory(ICON_PNG).ok().map(|img| {
        let img = img.into_rgba8();
        let (width, height) = img.dimensions();
        Arc::new(egui::IconData { rgba: img.into_raw(), width, height })
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("GameRec")
        .with_app_id("GameRec")
        .with_inner_size([1000.0, 700.0])
        .with_min_inner_size([900.0, 620.0])
        .with_drag_and_drop(false);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    if let Err(e) = window::run(viewport, ui::GameRecApp::new) {
        win::message_box("GameRec", &format!("Não foi possível abrir a janela: {e}"));
    }
}
