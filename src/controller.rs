//! Thread de gravacao: inicia/para captura, independente da interface
//! (o F9 funciona mesmo com a janela minimizada e o jogo em tela cheia).
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio::{self, AudioRecorder};
use crate::config::Config;
use crate::video::{self, AudioTrack, Capture, Source, VideoRecorder};
use crate::win::{self, WindowInfo};

pub enum Cmd {
    /// Inicia ou para. Vem com a janela em foco quando disparado pelo F9.
    Toggle(Option<isize>),
    Quit,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Idle,
    Starting,
    Recording,
    Saving,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Muted,
    Live,
    Warn,
    Ok,
}

#[derive(Clone)]
pub struct Status {
    pub phase: Phase,
    pub headline: String,
    pub tone: Tone,
    pub info: String,
    /// explicacao completa do ultimo erro (painel com "Copiar")
    pub detail: Option<String>,
    /// aviso durante a gravacao (ex.: microfone falhou, gravando sem ele)
    pub warning: Option<String>,
    pub rec_start: Option<Instant>,
    pub target: String,
    pub encoder: Option<&'static str>,
    pub sys_active: bool,
    pub mic_active: bool,
    /// incrementa a cada arquivo salvo (a lista de gravacoes recarrega)
    pub saved: u64,
}

pub struct Shared {
    pub status: Mutex<Status>,
    pub cfg: Mutex<Config>,
    pub sys_level: Arc<AtomicU32>,
    pub mic_level: Arc<AtomicU32>,
    pub fps: AtomicU32,
    pub main_hwnd: AtomicIsize,
}

impl Shared {
    pub fn new(cfg: Config) -> Self {
        Self {
            status: Mutex::new(Status {
                phase: Phase::Idle,
                headline: "PRONTO".into(),
                tone: Tone::Muted,
                info: "Pressione F9 em qualquer lugar para iniciar ou parar".into(),
                detail: None,
                warning: None,
                rec_start: None,
                target: String::new(),
                encoder: None,
                sys_active: false,
                mic_active: false,
                saved: 0,
            }),
            cfg: Mutex::new(cfg),
            sys_level: Arc::default(),
            mic_level: Arc::default(),
            fps: AtomicU32::new(0),
            main_hwnd: AtomicIsize::new(0),
        }
    }
}

struct Session {
    video: VideoRecorder,
    audios: Vec<(AudioRecorder, f32)>,
    tmp: PathBuf,
    video_path: PathBuf,
    final_path: PathBuf,
}

enum Target {
    Window(WindowInfo),
    Monitor(usize),
    Missing(String),
}

pub struct Controller {
    shared: Arc<Shared>,
    ctx: egui::Context,
}

impl Controller {
    pub fn new(shared: Arc<Shared>, ctx: egui::Context) -> Self {
        Self { shared, ctx }
    }

    fn update(&self, f: impl FnOnce(&mut Status)) {
        f(&mut self.shared.status.lock().unwrap());
        self.ctx.request_repaint();
    }

    fn set(&self, phase: Phase, headline: &str, tone: Tone, info: impl Into<String>) {
        let info = info.into();
        self.update(|s| {
            s.phase = phase;
            s.headline = headline.into();
            s.tone = tone;
            s.info = info;
            s.detail = None;
        });
    }

    fn fail(&self, headline: &str, info: impl Into<String>, detail: impl Into<String>) {
        let (info, detail) = (info.into(), detail.into());
        crate::log!("ERRO {headline}: {info}\n{detail}");
        self.update(|s| {
            s.phase = Phase::Idle;
            s.headline = headline.into();
            s.tone = Tone::Live;
            s.info = info;
            s.detail = Some(detail);
        });
    }

    pub fn run(self, rx: Receiver<Cmd>) {
        let mut session: Option<Session> = None;
        loop {
            let cmd = match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(c) => c,
                Err(RecvTimeoutError::Timeout) => {
                    if let Some(s) = session.as_mut() {
                        if s.video.alive() {
                            self.shared.fps.store(s.video.fps().to_bits(), Ordering::Relaxed);
                        } else {
                            // captura caiu (jogo fechou etc.): salva o que tiver
                            self.stop(session.take().unwrap());
                        }
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => Cmd::Quit,
            };
            match cmd {
                Cmd::Toggle(fg) => {
                    match session.take() {
                        Some(s) => self.stop(s),
                        None => session = self.start(fg),
                    }
                    // F9 apertado enquanto iniciava/salvava e ignorado
                    while let Ok(c) = rx.try_recv() {
                        if matches!(c, Cmd::Quit) {
                            if let Some(s) = session.take() {
                                self.stop(s);
                            }
                            return;
                        }
                    }
                }
                Cmd::Quit => {
                    if let Some(s) = session.take() {
                        self.stop(s);
                    }
                    return;
                }
            }
        }
    }

    fn resolve(&self, cfg: &Config, fg: Option<isize>) -> (Target, Vec<win::Monitor>) {
        let monitors = win::list_monitors();
        let (kind, val) = cfg.source.split_once(':').unwrap_or(("auto", ""));
        if kind == "mon" {
            let i = val.parse::<usize>().unwrap_or(0).min(monitors.len() - 1);
            return (Target::Monitor(i), monitors);
        }
        let windows = win::list_windows(&monitors);
        let target = if kind == "win" {
            match windows.iter().find(|w| w.exe == val) {
                Some(w) => Target::Window(w.clone()),
                None => Target::Missing(val.to_string()),
            }
        } else {
            match win::best_game(&windows, fg) {
                Some(w) => Target::Window(w.clone()),
                None => Target::Monitor(0),
            }
        };
        (target, monitors)
    }

    fn start(&self, fg: Option<isize>) -> Option<Session> {
        let cfg = self.shared.cfg.lock().unwrap().clone();
        let Some(encoder) = self.shared.status.lock().unwrap().encoder else {
            self.update(|s| s.info = "Aguarde a detecção do encoder...".into());
            return None;
        };
        let (target, monitors) = self.resolve(&cfg, fg);
        let (win, mon_idx) = match target {
            Target::Missing(exe) => {
                self.set(
                    Phase::Idle,
                    "JANELA NÃO ENCONTRADA",
                    Tone::Live,
                    format!("{exe} não está aberto. Abra o jogo ou escolha outra fonte."),
                );
                return None;
            }
            Target::Window(w) if w.iconic => {
                self.set(
                    Phase::Idle,
                    "JOGO MINIMIZADO",
                    Tone::Warn,
                    format!("Abra \"{}\" e aperte F9 dentro do jogo para gravar.", w.title),
                );
                return None;
            }
            Target::Window(w) => (Some(w), 0),
            Target::Monitor(i) => (None, i),
        };
        let mon = &monitors[mon_idx];
        let target_name = win.as_ref().map_or_else(|| format!("Monitor {}", mon_idx + 1), |w| w.title.clone());
        self.update(|s| {
            s.phase = Phase::Starting;
            s.headline = "INICIANDO".into();
            s.tone = Tone::Warn;
            s.info = format!("Preparando: {target_name}");
            s.detail = None;
            s.warning = None;
        });
        crate::log!(
            "iniciando: alvo={target_name} ({}) encoder={encoder} fps={} qualidade={} resolucao={} sistema={} mic={} ({})",
            win.as_ref().map_or("monitor", |w| w.exe.as_str()),
            cfg.fps,
            cfg.quality,
            cfg.resolution,
            cfg.system_audio,
            cfg.mic,
            cfg.mic_device
        );

        let out_dir = PathBuf::from(&cfg.output_dir);
        let stamp = win::local_stamp();
        let tmp = out_dir.join(format!(".gamerec_{stamp}"));
        if let Err(e) = std::fs::create_dir_all(&tmp) {
            self.fail(
                "ERRO NA PASTA",
                "Não foi possível criar a pasta de vídeos.",
                format!(
                    "Pasta: {}\nErro: {e}\n\nEscolha outra pasta em Saída > Alterar (pastas do OneDrive ou \
                     protegidas contra ransomware costumam bloquear).",
                    out_dir.display()
                ),
            );
            return None;
        }

        // audio primeiro (o video e alinhado depois pelos timestamps); falha de audio nao impede gravar
        let mut audios = Vec::new();
        let mut warnings = Vec::new();
        let devices = audio::devices();
        let (mut sys_on, mut mic_on) = (false, false);
        if cfg.system_audio {
            match &devices.output {
                Some(out) => match AudioRecorder::loopback(out, &tmp.join("system.wav"), self.shared.sys_level.clone())
                {
                    Ok(r) => {
                        audios.push((r, cfg.system_volume));
                        sys_on = true;
                    }
                    Err(e) => warnings.push(format!("Som do computador não foi gravado: {e}")),
                },
                None => warnings.push("Som do computador não foi gravado: nenhuma saída de áudio ativa.".into()),
            }
        }
        if cfg.mic {
            let dev = devices.mics.iter().find(|(n, _)| *n == cfg.mic_device).or(devices.mics.first());
            match dev {
                Some((name, dev)) => {
                    match AudioRecorder::input(dev, &tmp.join("mic.wav"), self.shared.mic_level.clone()) {
                        Ok(r) => {
                            audios.push((r, cfg.mic_volume));
                            mic_on = true;
                        }
                        Err(e) => warnings.push(format!(
                        "Microfone \"{name}\" não foi gravado: {e}. Confira Configurações > Privacidade > Microfone."
                    )),
                    }
                }
                None => warnings.push("Microfone não foi gravado: nenhum microfone encontrado.".into()),
            }
        }
        for w in &warnings {
            crate::log!("aviso: {w}");
        }

        let video_path = tmp.join("video.mkv");
        let sources: &[Source] = if win.is_some() {
            &[Source::GfxWindow, Source::GdiWindow]
        } else {
            &[Source::GfxMonitor, Source::Ddagrab, Source::GdiDesktop]
        };
        let mut encoders = vec![encoder];
        if encoder != "libx264" {
            encoders.push("libx264");
        }
        let mut video = None;
        let mut attempts = Vec::new();
        'outer: for &source in sources {
            for &enc in &encoders {
                let cap = Capture {
                    source,
                    encoder: enc,
                    monitor_index: mon_idx,
                    monitor: mon,
                    hwnd: win.as_ref().map_or(0, |w| w.hwnd),
                    fps: cfg.fps(),
                    max_height: cfg.max_height(),
                    crf: cfg.crf(),
                    out: &video_path,
                };
                let mut v = match VideoRecorder::start(&cap) {
                    Ok(v) => v,
                    Err(e) => {
                        drop(audios);
                        let _ = std::fs::remove_dir_all(&tmp);
                        self.fail(
                            "FFMPEG NÃO ABRIU",
                            "O motor de gravação (FFmpeg) não pôde ser executado.",
                            format!("Arquivo: {}\nErro: {e}\n\n{}", video::ffmpeg_path().display(), spawn_hint(&e)),
                        );
                        return None;
                    }
                };
                let t = Instant::now();
                while t.elapsed() < Duration::from_secs(4) && v.alive() && v.frames() == 0 {
                    std::thread::sleep(Duration::from_millis(50));
                }
                if v.alive() && v.frames() > 0 {
                    video = Some(v);
                    break 'outer;
                }
                v.kill();
                let stderr = v.stderr();
                crate::log!("falhou {} + {enc}:\n{}", source.label(), stderr.trim());
                let why = video::summarize_error(&stderr);
                let why = if why.is_empty() { "nenhum quadro capturado em 4 s".to_string() } else { why };
                attempts.push(format!("• {} + {}: {why}", source.label(), video::encoder_name(enc)));
            }
        }

        let Some(video) = video else {
            drop(audios);
            let _ = std::fs::remove_dir_all(&tmp);
            self.fail(
                "ERRO NA CAPTURA",
                format!("Não foi possível capturar {target_name}."),
                format!(
                    "Tentativas:\n{}\n\nTente gravar outra fonte (ex.: Tela: Monitor 1), atualizar o driver \
                     da placa de vídeo ou reinstalar o GameRec.",
                    attempts.join("\n")
                ),
            );
            return None;
        };
        crate::log!("gravando ({} tentativa(s) falharam antes)", attempts.len());

        let warning = (!warnings.is_empty()).then(|| warnings.join("\n"));
        self.update(|s| {
            s.phase = Phase::Recording;
            s.headline = "GRAVANDO".into();
            s.tone = Tone::Live;
            s.info = format!("Gravando: {target_name}");
            s.rec_start = Some(Instant::now());
            s.target = target_name.clone();
            s.sys_active = sys_on;
            s.mic_active = mic_on;
            s.warning = warning;
        });
        if cfg.minimize {
            win::minimize(self.shared.main_hwnd.load(Ordering::Relaxed));
        }
        Some(Session { video, audios, tmp, video_path, final_path: out_dir.join(format!("Gameplay_{stamp}.mp4")) })
    }

    fn stop(&self, mut s: Session) {
        self.set(Phase::Saving, "PROCESSANDO", Tone::Warn, "Salvando vídeo...");
        crate::log!("parando");
        s.video.stop();
        let v0 = s.video.start_time();
        let mut tracks = Vec::new();
        for (rec, vol) in s.audios.iter_mut() {
            rec.stop();
            let (Some(a0), Some(v0)) = (rec.start_time(), v0) else { continue };
            if std::fs::metadata(&rec.path).is_ok_and(|m| m.len() > 1000) {
                tracks.push(AudioTrack { wav: rec.path.clone(), offset: v0 - a0, volume: *vol });
            }
        }
        let result = video::mux(&s.video_path, &tracks, &s.final_path);
        match &result {
            Ok(()) => crate::log!("salvo: {}", s.final_path.display()),
            Err(e) => crate::log!("ERRO ao salvar: {e}"),
        }
        if result.is_ok() {
            let _ = std::fs::remove_dir_all(&s.tmp);
        }
        self.shared.sys_level.store(0, Ordering::Relaxed);
        self.shared.mic_level.store(0, Ordering::Relaxed);
        let name = s.final_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.update(|st| {
            st.phase = Phase::Idle;
            st.rec_start = None;
            st.sys_active = false;
            st.mic_active = false;
            st.saved += 1;
            st.warning = None;
            match &result {
                Ok(()) => {
                    st.headline = "SALVO".into();
                    st.tone = Tone::Ok;
                    st.info = format!("Salvo: {name}");
                }
                Err(e) => {
                    st.headline = "ERRO AO SALVAR".into();
                    st.tone = Tone::Live;
                    st.info = "O vídeo foi gravado, mas a montagem do MP4 falhou.".into();
                    st.detail = Some(format!(
                        "Erro: {e}\n\nOs arquivos brutos (vídeo e áudio) ficaram em:\n{}",
                        s.tmp.display()
                    ));
                }
            }
        });
        win::restore(self.shared.main_hwnd.load(Ordering::Relaxed));
    }
}

/// Dica para quando o Windows nao deixa o FFmpeg rodar.
fn spawn_hint(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION / ERROR_ACCESS_DISABLED_BY_POLICY
        Some(4551) | Some(1260) => {
            "O Windows bloqueou o FFmpeg (Smart App Control ou política de segurança). Desligue o Smart App Control \
             em Segurança do Windows > Controle de aplicativos e navegador e reinstale o GameRec."
        }
        Some(2) | Some(3) => "O arquivo não existe. O antivírus pode ter apagado o FFmpeg: reinstale o GameRec.",
        Some(5) | Some(225) => {
            "Acesso negado: o antivírus bloqueou o FFmpeg. Libere o arquivo no antivírus e reinstale o GameRec."
        }
        _ => {
            "O antivírus pode ter apagado ou bloqueado o FFmpeg. Reinstale o GameRec ou libere o arquivo no antivírus."
        }
    }
}
