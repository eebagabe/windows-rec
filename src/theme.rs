//! Cores, fontes e widgets proprios da interface.
use std::sync::Arc;

use egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin,
    Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
};

use crate::controller::Tone;

pub const BG: Color32 = Color32::from_rgb(0x0b, 0x0d, 0x12);
pub const CARD: Color32 = Color32::from_rgb(0x14, 0x17, 0x20);
pub const CARD2: Color32 = Color32::from_rgb(0x1c, 0x20, 0x2b);
pub const CARD3: Color32 = Color32::from_rgb(0x26, 0x2b, 0x39);
pub const BORDER: Color32 = Color32::from_rgb(0x22, 0x26, 0x32);
pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x3b, 0x5c);
pub const PURPLE: Color32 = Color32::from_rgb(0x7a, 0x3c, 0xff);
pub const TEXT: Color32 = Color32::from_rgb(0xec, 0xee, 0xf4);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x90, 0xa0);
pub const DIM: Color32 = Color32::from_rgb(0x5a, 0x5f, 0x6e);
pub const GREEN: Color32 = Color32::from_rgb(0x2e, 0xcc, 0x71);
pub const ORANGE: Color32 = Color32::from_rgb(0xf5, 0xa6, 0x23);

/// Glifos da fonte Segoe MDL2 Assets (Windows 10/11).
pub mod icon {
    pub const PLAY: &str = "\u{E768}";
    pub const STOP: &str = "\u{E71A}";
    pub const RECORD: &str = "\u{E7C8}";
    pub const FOLDER: &str = "\u{E838}";
    pub const REFRESH: &str = "\u{E72C}";
    pub const MIC: &str = "\u{E720}";
    pub const VOLUME: &str = "\u{E767}";
    pub const VIDEO: &str = "\u{E714}";
    pub const GAME: &str = "\u{E7FC}";
    pub const BOLT: &str = "\u{E945}";
    pub const OPEN: &str = "\u{E8A7}";
}

pub fn tone_color(t: Tone) -> Color32 {
    match t {
        Tone::Muted => MUTED,
        Tone::Live => ACCENT,
        Tone::Warn => ORANGE,
        Tone::Ok => GREEN,
    }
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Usa as fontes do proprio Windows (Segoe UI + icones); nada embutido no exe.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir =
        std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let mut load = |name: &str, files: &[&str]| -> bool {
        for f in files {
            if let Ok(bytes) = std::fs::read(dir.join(f)) {
                fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };
    let ui_font = load("segoe", &["segoeui.ttf"]);
    let semi = load("segoe-semibold", &["seguisb.ttf", "segoeuib.ttf"]);
    let bold_font = load("segoe-bold", &["segoeuib.ttf", "seguisb.ttf"]);
    let icons = load("icons", &["SegoeIcons.ttf", "segmdl2.ttf"]);

    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |main: Option<&str>| -> Vec<String> {
        let mut v: Vec<String> = main.map(String::from).into_iter().collect();
        v.extend(base.iter().cloned());
        if icons {
            v.push("icons".into());
        }
        v
    };
    let prop = family(ui_font.then_some("segoe"));
    let semi_f = family(semi.then_some("segoe-semibold"));
    let bold_f = family(bold_font.then_some("segoe-bold"));
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name("semibold".into()), semi_f);
    fonts.families.insert(FontFamily::Name("bold".into()), bold_f);
    ctx.set_fonts(fonts);
}

pub fn setup_style(ctx: &egui::Context) {
    ctx.global_style_mut(|s| {
        use egui::TextStyle::*;
        s.text_styles = [
            (Heading, regular(24.0)),
            (Body, regular(14.0)),
            (Button, regular(14.0)),
            (Small, regular(12.0)),
            (Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.interact_size.y = 32.0;
        s.spacing.combo_height = 320.0;
        s.interaction.selectable_labels = false;

        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = CARD2;
        v.window_stroke = Stroke::new(1.0_f32, CARD3);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = CARD2;
        v.faint_bg_color = CARD2;
        v.selection.bg_fill = ACCENT;
        v.selection.stroke = Stroke::new(1.0_f32, TEXT);
        v.hyperlink_color = ACCENT;
        v.slider_trailing_fill = true;
        v.popup_shadow =
            egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(120) };
        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
        for (st, bg, fg) in [
            (&mut w.inactive, CARD2, TEXT),
            (&mut w.hovered, CARD3, TEXT),
            (&mut w.active, CARD3, TEXT),
            (&mut w.open, CARD3, TEXT),
        ] {
            st.bg_fill = bg;
            st.weak_bg_fill = bg;
            st.bg_stroke = Stroke::NONE;
            st.fg_stroke = Stroke::new(1.5_f32, fg);
            st.corner_radius = r;
            st.expansion = 0.0;
        }
        w.hovered.bg_stroke = Stroke::new(1.0_f32, CARD3);
    });
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn section_title(ui: &mut Ui, glyph: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(glyph).font(regular(14.0)).color(ACCENT));
        ui.label(egui::RichText::new(text).font(semibold(12.5)).color(MUTED));
    });
    ui.add_space(4.0);
}

pub fn field_label(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).font(regular(13.0)).color(MUTED));
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Pilula com gradiente horizontal (malha em leque, borda suavizada por baixo).
pub fn gradient_pill(ui: &Ui, rect: Rect, left: Color32, right: Color32) {
    let painter = ui.painter();
    let r = rect.height() / 2.0;
    painter.rect_filled(rect, CornerRadius::same(r as u8), lerp_color(left, right, 0.5));
    let inner = rect.shrink(0.6);
    let r = inner.height() / 2.0;
    let color_at = |p: Pos2| lerp_color(left, right, ((p.x - inner.left()) / inner.width()).clamp(0.0, 1.0));
    let mut points = Vec::new();
    let steps = 24;
    for (cx, a0) in [(inner.right() - r, -90.0f32), (inner.left() + r, 90.0)] {
        for i in 0..=steps {
            let a = (a0 + 180.0 * i as f32 / steps as f32).to_radians();
            points.push(pos2(cx + r * a.cos(), inner.center().y + r * a.sin()));
        }
    }
    let mut mesh = Mesh::default();
    mesh.colored_vertex(inner.center(), color_at(inner.center()));
    for p in &points {
        mesh.colored_vertex(*p, color_at(*p));
    }
    let n = points.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

/// Botao arredondado simples (secundario).
pub fn ghost_button(ui: &mut Ui, glyph: &str, text: &str) -> Response {
    let label = match (glyph.is_empty(), text.is_empty()) {
        (_, true) => glyph.to_string(),
        (true, false) => text.to_string(),
        (false, false) => format!("{glyph}   {text}"),
    };
    let galley = ui.painter().layout_no_wrap(label, regular(13.5), TEXT);
    let size = vec2(galley.size().x + 26.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool(resp.id, resp.hovered() && ui.is_enabled());
    let fill = lerp_color(CARD2, CARD3, t);
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    let color = if ui.is_enabled() { TEXT } else { DIM };
    ui.painter().galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Interruptor liga/desliga com texto.
pub fn toggle(ui: &mut Ui, on: &mut bool, text: &str) -> Response {
    let enabled = ui.is_enabled();
    let width = ui.available_width();
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let sw = Rect::from_min_size(pos2(rect.right() - 42.0, rect.center().y - 11.0), vec2(42.0, 22.0));
    let off_col = if resp.hovered() && enabled { CARD3 } else { CARD2 };
    let mut fill = lerp_color(off_col, ACCENT, t);
    let mut text_col = if *on { TEXT } else { MUTED };
    if !enabled {
        fill = fill.gamma_multiply(0.5);
        text_col = DIM;
    }
    let p = ui.painter();
    p.rect_filled(sw, CornerRadius::same(11), fill);
    let x = egui::lerp(sw.left() + 11.0..=sw.right() - 11.0, t);
    p.circle_filled(pos2(x, sw.center().y), 8.0, if enabled { Color32::WHITE } else { MUTED });
    p.text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, text, regular(14.0), text_col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Controle segmentado (uma opcao entre varias).
pub fn segmented(ui: &mut Ui, id: &str, value: &mut String, options: &[&str]) {
    let enabled = ui.is_enabled();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(9), CARD2);
    let seg_w = (rect.width() - 6.0) / options.len() as f32;
    for (i, opt) in options.iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(3.0 + i as f32 * seg_w, 3.0), vec2(seg_w, rect.height() - 6.0));
        let resp = ui.interact(r, ui.id().with((id, i)), Sense::click());
        if resp.clicked() {
            *value = opt.to_string();
        }
        let selected = value == opt;
        let t = ui.ctx().animate_bool_responsive(resp.id, selected);
        if t > 0.0 {
            let fill = if enabled { ACCENT } else { CARD3 };
            ui.painter().rect_filled(r, CornerRadius::same(7), fill.gamma_multiply(t));
        } else if resp.hovered() && enabled {
            ui.painter().rect_filled(r, CornerRadius::same(7), CARD3);
        }
        let color = match (enabled, selected) {
            (false, _) => DIM,
            (true, true) => Color32::WHITE,
            (true, false) => MUTED,
        };
        let font = if selected { semibold(13.5) } else { regular(13.5) };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *opt, font, color);
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
}

/// Slider de volume 0-200% com valor ao lado.
pub fn volume_slider(ui: &mut Ui, value: &mut f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = ui.available_width() - 56.0;
        ui.spacing_mut().slider_rail_height = 6.0;
        ui.add(egui::Slider::new(value, 0.0..=200.0).step_by(5.0).show_value(false));
        ui.label(egui::RichText::new(format!("{:.0}%", value)).font(regular(13.0)).color(MUTED));
    });
}

/// Medidor de nivel de audio em segmentos (escala em dB).
pub fn meter(ui: &mut Ui, glyph: &str, label: &str, level: f32, active: bool) {
    ui.horizontal(|ui| {
        let col = if active { TEXT } else { DIM };
        ui.label(egui::RichText::new(glyph).font(regular(14.0)).color(col));
        let (lr, _) = ui.allocate_exact_size(vec2(72.0, 18.0), Sense::hover());
        ui.painter().text(lr.left_center(), Align2::LEFT_CENTER, label, regular(13.0), col);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
        // -60 dB..0 dB
        let db = if level > 0.0 { 20.0 * level.log10() } else { -100.0 };
        let fill = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let n = 36;
        let gap = 2.5;
        let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
        let lit = (fill * n as f32).round() as usize;
        for i in 0..n {
            let x = rect.left() + i as f32 * (w + gap);
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
            let f = i as f32 / n as f32;
            let on_col = if f < 0.7 {
                GREEN
            } else if f < 0.88 {
                ORANGE
            } else {
                ACCENT
            };
            let c = if i < lit { on_col } else { CARD2 };
            ui.painter().rect_filled(r, CornerRadius::same(2), c);
        }
    });
}

/// Pilula de status (ponto + texto).
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32, dot_alpha: f32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), semibold(12.5), color);
    let size = vec2(galley.size().x + 34.0, 28.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(14), color.gamma_multiply(0.14));
    p.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0_f32, color.gamma_multiply(0.35)), StrokeKind::Inside);
    p.circle_filled(pos2(rect.left() + 15.0, rect.center().y), 4.5, color.gamma_multiply(dot_alpha));
    p.galley(pos2(rect.left() + 25.0, rect.center().y - galley.size().y / 2.0), galley, color);
}
