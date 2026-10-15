//! Interface (egui). So desenha e envia comandos; a gravacao roda no `controller`.
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, RichText, Sense,
    TextureHandle, Ui,
};

use crate::config::{Config, FPS, QUALITY, RESOLUTIONS};
use crate::controller::{Cmd, Controller, Phase, Shared, Status};
use crate::theme::{self, icon, *};
use crate::{audio, video, win};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

const AUTO_KEY: &str = "auto:None";
const AUTO_LABEL: &str = "Automático (jogo aberto)";
const VK_F9: u32 = 0x78;
const MAX_FILES: usize = 8;

struct FileEntry {
    path: PathBuf,
    name: String,
    detail: String,
}

pub struct GameRecApp {
    shared: Arc<Shared>,
    tx: Sender<Cmd>,
    controller: Option<JoinHandle<()>>,
    cfg: Config,
    saved_cfg: Config,
    sources: Vec<(String, String)>,
    detect: (String, Color32),
    last_scan: Instant,
    mic_names: Vec<String>,
    has_loopback: bool,
    hotkey_ok: bool,
    files: Vec<FileEntry>,
    files_rev: u64,
    logo: Option<TextureHandle>,
    window: Arc<Window>,
    meter_sys: f32,
    meter_mic: f32,
    copied_at: Option<Instant>,
}

impl GameRecApp {
    pub fn new(ctx: egui::Context, window: Arc<Window>) -> Self {
        theme::setup_fonts(&ctx);
        theme::setup_style(&ctx);
        let cfg = Config::load();

        // enumera audio fora da thread da janela (COM/STA do winit)
        let (mic_names, has_loopback) = std::thread::spawn(|| {
            let d = audio::devices();
            (d.mics.into_iter().map(|(n, _)| n).collect::<Vec<_>>(), d.output.is_some())
        })
        .join()
        .unwrap_or_default();
        crate::log!("audio: saida={has_loopback} microfones={mic_names:?}; pasta={}", cfg.output_dir);

        let shared = Arc::new(Shared::new(cfg.clone()));
        if let Ok(h) = window.window_handle() {
            if let RawWindowHandle::Win32(w) = h.as_raw() {
                shared.main_hwnd.store(w.hwnd.get(), Ordering::Relaxed);
            }
        }

        let (tx, rx) = mpsc::channel();
        let ctl = Controller::new(shared.clone(), ctx.clone());
        let controller = std::thread::Builder::new().name("controller".into()).spawn(move || ctl.run(rx)).ok();

        let (sh, repaint) = (shared.clone(), ctx.clone());
        std::thread::spawn(move || {
            let enc = video::detect_encoder();
            crate::log!("encoder: {enc}");
            sh.status.lock().unwrap().encoder = Some(enc);
            repaint.request_repaint();
        });

        let htx = tx.clone();
        let hotkey_ok = win::spawn_hotkey(VK_F9, move || {
            // janela em foco no momento do F9 (o jogo)
            let _ = htx.send(Cmd::Toggle(Some(win::foreground_window())));
        });

        let logo = image::load_from_memory(crate::ICON_PNG).ok().map(|img| {
            let img = img.into_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            ctx.load_texture("logo", color, egui::TextureOptions::LINEAR)
        });

        let mut app = Self {
            shared,
            tx,
            controller,
            saved_cfg: cfg.clone(),
            cfg,
            sources: Vec::new(),
            detect: (String::new(), MUTED),
            last_scan: Instant::now(),
            mic_names,
            has_loopback,
            hotkey_ok,
            files: Vec::new(),
            files_rev: 0,
            logo,
            window,
            meter_sys: 0.0,
            meter_mic: 0.0,
            copied_at: None,
        };
        if !app.has_loopback {
            app.cfg.system_audio = false;
        }
        if app.mic_names.is_empty() {
            app.cfg.mic = false;
        } else if !app.mic_names.contains(&app.cfg.mic_device) {
            app.cfg.mic_device = app.mic_names[0].clone();
        }
        app.refresh_sources();
        app.refresh_files();
        app
    }

    // ------------------------------------------------------------ dados
    fn refresh_sources(&mut self) {
        let monitors = win::list_monitors();
        let windows = win::list_windows(&monitors);
        let mut sources = vec![(AUTO_LABEL.to_string(), AUTO_KEY.to_string())];
        for (i, m) in monitors.iter().enumerate() {
            let main = if m.primary { " (principal)" } else { "" };
            sources.push((format!("Tela: Monitor {} - {}x{}{main}", i + 1, m.w, m.h), format!("mon:{i}")));
        }
        let mut seen = std::collections::HashSet::new();
        for w in &windows {
            if !seen.insert(w.exe.clone()) {
                continue;
            }
            let kind = if w.score >= 2 { "Jogo:" } else { "Janela:" };
            sources.push((format!("{kind} {}", truncate(&w.title, 34)), format!("win:{}", w.exe)));
        }
        // fonte salva que nao esta aberta agora continua selecionavel
        if !sources.iter().any(|(_, k)| *k == self.cfg.source) {
            match self.cfg.source.strip_prefix("win:") {
                Some(exe) => sources.push((format!("Janela: {exe} (fechado)"), self.cfg.source.clone())),
                None => self.cfg.source = AUTO_KEY.into(),
            }
        }
        self.sources = sources;
        self.detect = match win::best_game(&windows, None) {
            Some(g) if g.iconic => (format!("Jogo detectado: {}  (minimizado)", g.title), ORANGE),
            Some(g) => (format!("Jogo detectado: {}", g.title), GREEN),
            None => ("Nenhum jogo aberto · Automático grava o monitor principal".into(), MUTED),
        };
        self.last_scan = Instant::now();
    }

    fn refresh_files(&mut self) {
        let mut vids: Vec<(SystemTime, PathBuf, u64)> = std::fs::read_dir(&self.cfg.output_dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("mp4")))
                    .filter_map(|e| {
                        let m = e.metadata().ok()?;
                        Some((m.modified().ok()?, e.path(), m.len()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        vids.sort_by_key(|v| std::cmp::Reverse(v.0));
        self.files = vids
            .into_iter()
            .take(MAX_FILES)
            .map(|(_, path, len)| {
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let size = len as f64 / 1024.0 / 1024.0;
                let size = if size >= 1024.0 { format!("{:.1} GB", size / 1024.0) } else { format!("{size:.0} MB") };
                let detail = match stamp_from_name(&name) {
                    Some(when) => format!("{when}  ·  {size}"),
                    None => size,
                };
                FileEntry { path, name, detail }
            })
            .collect();
    }

    fn sync_cfg(&mut self, ctx: &egui::Context) {
        if self.cfg != *self.shared.cfg.lock().unwrap() {
            *self.shared.cfg.lock().unwrap() = self.cfg.clone();
        }
        // grava no disco quando o usuario solta o controle
        if self.cfg != self.saved_cfg && !ctx.input(|i| i.pointer.any_down()) {
            self.cfg.save();
            self.saved_cfg = self.cfg.clone();
        }
    }

    fn choose_folder(&mut self) {
        let dlg = rfd::FileDialog::new().set_directory(&self.cfg.output_dir).set_parent(&*self.window);
        if let Some(dir) = dlg.pick_folder() {
            self.cfg.output_dir = dir.to_string_lossy().into_owned();
            self.refresh_files();
        }
    }

    fn open_folder(&self) {
        let _ = std::fs::create_dir_all(&self.cfg.output_dir);
        win::open(&self.cfg.output_dir);
    }

    // ------------------------------------------------------------ layout
    fn header(&self, ui: &mut Ui, st: &Status) {
        ui.horizontal(|ui| {
            if let Some(logo) = &self.logo {
                let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                ui.painter().image(logo.id(), rect, uv, Color32::WHITE);
                ui.add_space(4.0);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("GameRec").font(bold(24.0)).color(TEXT));
                ui.label(RichText::new("Gravador de gameplay para YouTube").font(regular(13.0)).color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (txt, col) = match st.encoder {
                    Some(enc) if enc != "libx264" => (format!("{}  {}", icon::BOLT, video::encoder_name(enc)), GREEN),
                    Some(enc) => (format!("{}  {}", icon::BOLT, video::encoder_name(enc)), MUTED),
                    None => ("Detectando encoder...".to_string(), MUTED),
                };
                chip(ui, &txt, col);
                let (hk, hk_col) =
                    if self.hotkey_ok { ("F9  iniciar / parar", MUTED) } else { ("F9 indisponível", ORANGE) };
                chip(ui, hk, hk_col).on_hover_text(if self.hotkey_ok {
                    "Funciona em qualquer lugar, inclusive com o jogo em tela cheia"
                } else {
                    "Outro programa (ou outro GameRec aberto) já usa o F9"
                });
            });
        });
    }

    fn hero(&mut self, ui: &mut Ui, st: &Status) {
        let blink = st.rec_start.map_or(1.0, |t| {
            let phase = (t.elapsed().as_secs_f32() * std::f32::consts::PI).sin();
            0.35 + 0.65 * phase.abs()
        });
        card(ui, |ui| {
            ui.horizontal(|ui| {
                status_pill(ui, &st.headline, tone_color(st.tone), blink);
                if st.phase == Phase::Recording {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let fps = f32::from_bits(self.shared.fps.load(Ordering::Relaxed));
                        chip(ui, &format!("{fps:.0} fps"), MUTED);
                    });
                }
            });
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                let elapsed = st.rec_start.map_or(0, |t| t.elapsed().as_secs());
                let color = if st.phase == Phase::Recording { TEXT } else { theme::MUTED };
                ui.label(RichText::new(fmt_time(elapsed)).font(bold(68.0)).color(color));
                ui.add_space(-6.0);
                if st.phase == Phase::Recording {
                    let info = format!("{}  ·  F9 para parar", st.target);
                    ui.add(egui::Label::new(RichText::new(info).font(regular(13.5)).color(MUTED)).truncate());
                } else {
                    ui.add(egui::Label::new(RichText::new(&st.info).font(regular(13.5)).color(MUTED)).wrap());
                }
                ui.add_space(14.0);
                if self.record_button(ui, st.phase).clicked() {
                    let _ = self.tx.send(Cmd::Toggle(None));
                }
            });
            let notice = match st.phase {
                Phase::Idle => st.detail.as_ref().map(|d| (d, tone_color(st.tone))),
                Phase::Recording => st.warning.as_ref().map(|w| (w, ORANGE)),
                _ => None,
            };
            if let Some((text, color)) = notice {
                ui.add_space(16.0);
                self.notice(ui, &st.headline, &st.info, text, color);
            }
            ui.add_space(18.0);
            let live = st.phase == Phase::Recording;
            let decay = |m: &mut f32, raw: u32, active: bool| {
                let lv = if active { f32::from_bits(raw) } else { 0.0 };
                *m = lv.max(*m * 0.82);
            };
            decay(&mut self.meter_sys, self.shared.sys_level.swap(0, Ordering::Relaxed), live && st.sys_active);
            decay(&mut self.meter_mic, self.shared.mic_level.swap(0, Ordering::Relaxed), live && st.mic_active);
            ui.add_space(2.0);
            meter(ui, icon::VOLUME, "Sistema", self.meter_sys, live && st.sys_active);
            meter(ui, icon::MIC, "Microfone", self.meter_mic, live && st.mic_active);
        });
    }

    /// Painel com a explicacao completa do erro/aviso, para copiar e mandar.
    fn notice(&mut self, ui: &mut Ui, headline: &str, info: &str, text: &str, color: Color32) {
        Frame::new()
            .fill(color.gamma_multiply(0.08))
            .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.35)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text).font(regular(13.0)).color(TEXT)).wrap());
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let copied = self.copied_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
                    if ghost_button(ui, "", if copied { "Copiado!" } else { "Copiar erro" }).clicked() {
                        let log = crate::log::path();
                        ui.ctx().copy_text(format!(
                            "GameRec {} — {headline}\n{info}\n\n{text}\n\nLog: {}",
                            env!("CARGO_PKG_VERSION"),
                            log.display()
                        ));
                        self.copied_at = Some(Instant::now());
                        ui.ctx().request_repaint_after(Duration::from_secs(2));
                    }
                    if ghost_button(ui, "", "Abrir log")
                        .on_hover_text(crate::log::path().display().to_string())
                        .clicked()
                    {
                        win::open(&crate::log::path().to_string_lossy());
                    }
                });
            });
    }

    fn record_button(&self, ui: &mut Ui, phase: Phase) -> egui::Response {
        let w = ui.available_width().min(400.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
        let enabled = matches!(phase, Phase::Idle | Phase::Recording);
        let hover = ui.ctx().animate_bool(resp.id, resp.hovered() && enabled);
        let radius = CornerRadius::same(29);
        let p = ui.painter();
        let (label, text_col) = match phase {
            Phase::Idle => {
                let lift = |c: Color32| c.lerp_to_gamma(Color32::WHITE, 0.12 * hover);
                if hover > 0.0 {
                    // brilho suave em volta
                    p.rect_filled(
                        rect.expand(3.0 * hover),
                        CornerRadius::same(32),
                        ACCENT.gamma_multiply(0.18 * hover),
                    );
                }
                theme::gradient_pill(ui, rect, lift(ACCENT), lift(PURPLE));
                (format!("{}   INICIAR GRAVAÇÃO", icon::RECORD), Color32::WHITE)
            }
            Phase::Recording => {
                p.rect_filled(rect, radius, CARD2.lerp_to_gamma(CARD3, hover));
                p.rect_stroke(rect, radius, egui::Stroke::new(1.5_f32, ACCENT), egui::StrokeKind::Inside);
                (format!("{}   PARAR GRAVAÇÃO", icon::STOP), TEXT)
            }
            Phase::Starting | Phase::Saving => {
                p.rect_filled(rect, radius, CARD2);
                let s = if phase == Phase::Starting { "Iniciando..." } else { "Salvando vídeo..." };
                (s.to_string(), MUTED)
            }
        };
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, semibold(16.5), text_col);
        if matches!(phase, Phase::Starting | Phase::Saving) {
            let r = Rect::from_center_size(pos2(rect.left() + 34.0, rect.center().y), vec2(20.0, 20.0));
            ui.put(r, egui::Spinner::new().size(18.0).color(MUTED));
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }

    fn recent(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                theme::section_title(ui, icon::VIDEO, "GRAVAÇÕES RECENTES");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ghost_button(ui, icon::OPEN, "YouTube Studio").clicked() {
                        win::open("https://studio.youtube.com");
                    }
                    if ghost_button(ui, icon::FOLDER, "Abrir pasta").clicked() {
                        self.open_folder();
                    }
                });
            });
            ui.add_space(4.0);
            if self.files.is_empty() {
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(icon::VIDEO).font(regular(28.0)).color(DIM));
                    ui.label(RichText::new("Nenhuma gravação ainda").color(MUTED));
                    ui.label(RichText::new("Abra um jogo e aperte F9").font(regular(12.5)).color(DIM));
                });
                ui.add_space(18.0);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                for f in &self.files {
                    if file_row(ui, f).clicked() {
                        win::open(&f.path.to_string_lossy());
                    }
                }
            });
        });
    }

    fn settings(&mut self, ui: &mut Ui, idle: bool) {
        card(ui, |ui| {
            ui.add_enabled_ui(idle, |ui| {
                theme::section_title(ui, icon::GAME, "VÍDEO");
                field_label(ui, "Gravar");
                // botao primeiro (direita); o combo ocupa o resto
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 32.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        if ghost_button(ui, icon::REFRESH, "").on_hover_text("Atualizar lista").clicked() {
                            self.refresh_sources();
                        }
                        let selected = self
                            .sources
                            .iter()
                            .find(|(_, k)| *k == self.cfg.source)
                            .map_or(AUTO_LABEL, |(l, _)| l.as_str())
                            .to_string();
                        egui::ComboBox::from_id_salt("source")
                            .width(ui.available_width())
                            .truncate()
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for (label, key) in &self.sources {
                                    ui.selectable_value(&mut self.cfg.source, key.clone(), label);
                                }
                            });
                    },
                );
                let (txt, col) = &self.detect;
                ui.add(egui::Label::new(RichText::new(txt).font(regular(12.5)).color(*col)).truncate());
                field_label(ui, "FPS");
                segmented(ui, "fps", &mut self.cfg.fps, &FPS);
                field_label(ui, "Resolução");
                let res: Vec<&str> = RESOLUTIONS.iter().map(|(k, _)| *k).collect();
                segmented(ui, "res", &mut self.cfg.resolution, &res);
                field_label(ui, "Qualidade");
                let q: Vec<&str> = QUALITY.iter().map(|(k, _)| *k).collect();
                segmented(ui, "quality", &mut self.cfg.quality, &q);
            });
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.add_enabled_ui(idle, |ui| {
                theme::section_title(ui, icon::VOLUME, "ÁUDIO");
                ui.add_enabled_ui(self.has_loopback, |ui| {
                    toggle(ui, &mut self.cfg.system_audio, "Som do computador (jogo)");
                    ui.add_enabled_ui(self.cfg.system_audio, |ui| volume_slider(ui, &mut self.cfg.system_volume));
                });
                ui.add_space(6.0);
                ui.add_enabled_ui(!self.mic_names.is_empty(), |ui| {
                    toggle(ui, &mut self.cfg.mic, "Microfone");
                    ui.add_enabled_ui(self.cfg.mic, |ui| {
                        let shown = if self.mic_names.is_empty() {
                            "Nenhum microfone encontrado".to_string()
                        } else {
                            truncate(&self.cfg.mic_device, 40)
                        };
                        egui::ComboBox::from_id_salt("mic")
                            .width(ui.available_width())
                            .truncate()
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for name in &self.mic_names {
                                    ui.selectable_value(&mut self.cfg.mic_device, name.clone(), name);
                                }
                            });
                        volume_slider(ui, &mut self.cfg.mic_volume);
                    });
                });
            });
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.add_enabled_ui(idle, |ui| {
                theme::section_title(ui, icon::FOLDER, "SAÍDA");
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 92.0;
                    ui.allocate_ui(vec2(w, 32.0), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&self.cfg.output_dir).font(regular(13.0)).color(TEXT))
                                .truncate(),
                        )
                        .on_hover_text(&self.cfg.output_dir);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, "", "Alterar").clicked() {
                            self.choose_folder();
                        }
                    });
                });
                ui.add_space(4.0);
                toggle(ui, &mut self.cfg.minimize, "Minimizar ao gravar");
            });
        });
    }
}

impl crate::window::App for GameRecApp {
    fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let st = self.shared.status.lock().unwrap().clone();
        let idle = st.phase == Phase::Idle;

        if st.saved != self.files_rev {
            self.files_rev = st.saved;
            self.refresh_files();
        }
        if idle && self.last_scan.elapsed() > Duration::from_secs(3) {
            self.refresh_sources();
        }

        let bg = Frame::new().fill(BG);
        egui::Panel::top("header")
            .show_separator_line(false)
            .frame(bg.inner_margin(Margin { left: 24, right: 24, top: 18, bottom: 12 }))
            .show_inside(ui, |ui| self.header(ui, &st));
        egui::Panel::right("settings")
            .exact_size(370.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(bg.inner_margin(Margin { left: 6, right: 24, top: 4, bottom: 20 }))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.settings(ui, idle));
            });
        egui::CentralPanel::default()
            .frame(bg.inner_margin(Margin { left: 24, right: 6, top: 4, bottom: 20 }))
            .show_inside(ui, |ui| {
                self.hero(ui, &st);
                ui.add_space(12.0);
                self.recent(ui);
            });

        self.sync_cfg(&ctx);
        // fora de foco (jogando) so o relogio importa: 1 quadro por segundo
        let focused = ctx.input(|i| i.focused);
        let next = match st.phase {
            Phase::Recording if !focused => Duration::from_secs(1),
            Phase::Recording | Phase::Starting | Phase::Saving => Duration::from_millis(100),
            Phase::Idle if self.meter_sys > 0.001 || self.meter_mic > 0.001 => Duration::from_millis(50),
            Phase::Idle => Duration::from_secs(3),
        };
        ctx.request_repaint_after(next);
    }

    /// Fechar no meio da gravacao: esconde a janela e salva antes de sair.
    fn on_close(&mut self) {
        let _ = self.tx.send(Cmd::Quit);
        if let Some(h) = self.controller.take() {
            let _ = h.join();
        }
        self.cfg.save();
    }
}

fn chip(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), regular(12.5), color);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 22.0, 28.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(14), CARD);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(14),
        egui::Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

fn file_row(ui: &mut Ui, f: &FileEntry) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let p = ui.painter();
    if hover > 0.0 {
        p.rect_filled(rect, CornerRadius::same(10), CARD2.gamma_multiply(hover));
    }
    let c = pos2(rect.left() + 26.0, rect.center().y);
    p.circle_filled(c, 15.0, ACCENT.gamma_multiply(0.15 + 0.85 * hover));
    let play_col = Color32::WHITE.lerp_to_gamma(ACCENT, 1.0 - hover);
    p.text(c + vec2(1.0, 0.0), Align2::CENTER_CENTER, icon::PLAY, regular(12.0), play_col);
    let x = rect.left() + 52.0;
    p.text(pos2(x, rect.center().y - 9.0), Align2::LEFT_CENTER, &f.name, regular(14.0), TEXT);
    p.text(pos2(x, rect.center().y + 10.0), Align2::LEFT_CENTER, &f.detail, regular(12.0), MUTED);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Abrir vídeo")
}

fn fmt_time(s: u64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

/// "Gameplay_2026-10-15_14-03-22.mp4" -> "15/10/2026 14:03"
fn stamp_from_name(name: &str) -> Option<String> {
    let s = Path::new(name).file_stem()?.to_str()?.strip_prefix("Gameplay_")?;
    let (date, time) = s.split_once('_')?;
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split('-').collect();
    (d.len() == 3 && t.len() == 3).then(|| format!("{}/{}/{} {}:{}", d[2], d[1], d[0], t[0], t[1]))
}
