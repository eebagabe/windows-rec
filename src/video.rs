//! Captura de video e mixagem final via FFmpeg (bin\ffmpeg.exe ao lado do app).
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::clock;
use crate::win::Monitor;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn encoder_name(enc: &str) -> &'static str {
    match enc {
        "h264_nvenc" => "NVIDIA NVENC",
        "h264_amf" => "AMD AMF",
        "h264_qsv" => "Intel Quick Sync",
        _ => "CPU (x264)",
    }
}

pub fn ffmpeg_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
        [dir.join("bin").join("ffmpeg.exe"), dir.join("ffmpeg.exe")]
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("ffmpeg.exe"))
    })
}

fn ffmpeg() -> Command {
    let mut c = Command::new(ffmpeg_path());
    c.creation_flags(CREATE_NO_WINDOW).arg("-hide_banner");
    c
}

fn wait_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let t = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) => return Some(s),
            Ok(None) if t.elapsed() < timeout => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Primeiro encoder de GPU que funciona; senao x264.
pub fn detect_encoder() -> &'static str {
    for enc in ["h264_nvenc", "h264_amf", "h264_qsv"] {
        let child = ffmpeg()
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "color=black:s=1280x720:r=30"])
            .args(["-frames:v", "10", "-pix_fmt", "nv12", "-c:v", enc, "-f", "null", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            if wait_timeout(&mut child, Duration::from_secs(20)).is_some_and(|s| s.success()) {
                return enc;
            }
        }
    }
    "libx264"
}

fn encoder_args(enc: &str, q: u32, fps: u32) -> Vec<String> {
    let gop = (fps * 2).to_string();
    let q2 = (q + 2).to_string();
    let q = q.to_string();
    let a: Vec<&str> = match enc {
        "h264_nvenc" => vec![
            "-c:v",
            enc,
            "-preset",
            "p4",
            "-tune",
            "hq",
            "-rc",
            "vbr",
            "-cq",
            &q,
            "-b:v",
            "0",
            "-maxrate",
            "100M",
            "-bufsize",
            "200M",
            "-profile:v",
            "high",
            "-g",
            &gop,
        ],
        "h264_amf" => vec![
            "-c:v",
            enc,
            "-quality",
            "balanced",
            "-rc",
            "cqp",
            "-qp_i",
            &q,
            "-qp_p",
            &q2,
            "-profile:v",
            "high",
            "-g",
            &gop,
        ],
        "h264_qsv" => vec!["-c:v", enc, "-preset", "medium", "-global_quality", &q, "-profile:v", "high", "-g", &gop],
        _ => vec!["-c:v", "libx264", "-preset", "veryfast", "-crf", &q, "-profile:v", "high", "-g", &gop],
    };
    a.into_iter().map(String::from).collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    /// Windows Graphics Capture de uma janela (sem o que estiver por cima).
    GfxWindow,
    GfxMonitor,
    Ddagrab,
    GdiWindow,
    GdiDesktop,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::GfxWindow => "captura de janela (WGC)",
            Source::GfxMonitor => "captura de tela (WGC)",
            Source::Ddagrab => "captura de tela (DXGI)",
            Source::GdiWindow => "captura de janela (GDI)",
            Source::GdiDesktop => "captura de tela (GDI)",
        }
    }
}

/// Mensagens do FFmpeg que so dizem "falhou", sem a causa.
const GENERIC: &[&str] = &[
    "conversion failed",
    "error opening output file",
    "error opening input file",
    "error opening output files",
    "error opening input files",
    "nothing was written into output file",
    "terminating thread with return code",
    "exiting normally",
];

/// Linha que explica o erro: a primeira que nao e generica, sem o "[filtro @ 0x...]".
pub fn summarize_error(stderr: &str) -> String {
    let clean = |l: &str| -> String {
        let l = l.trim();
        match (l.strip_prefix('['), l.find("] ")) {
            (Some(rest), Some(end)) => {
                let name = rest[..end - 1].split(" @ ").next().unwrap_or("");
                format!("{name}: {}", &l[end + 2..])
            }
            _ => l.to_string(),
        }
    };
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .iter()
        .find(|l| {
            let low = l.to_lowercase();
            !GENERIC.iter().any(|g| low.contains(g))
        })
        .or(lines.last())
        .map(|l| clean(l))
        .unwrap_or_default()
}

pub struct Capture<'a> {
    pub source: Source,
    pub encoder: &'a str,
    pub monitor_index: usize,
    pub monitor: &'a Monitor,
    pub hwnd: isize,
    pub fps: u32,
    pub max_height: Option<u32>,
    pub crf: u32,
    pub out: &'a Path,
}

fn video_args(c: &Capture) -> Vec<String> {
    let fps = c.fps.to_string();
    let mut a: Vec<String> = ["-loglevel", "error", "-nostats", "-y", "-progress", "pipe:1", "-stats_period", "0.1"]
        .map(String::from)
        .to_vec();
    let scale = match c.max_height {
        Some(h) => format!("scale=-2:'min(ih,{h})':flags=bicubic:out_color_matrix=bt709:out_range=tv"),
        None => "scale=out_color_matrix=bt709:out_range=tv".into(),
    };
    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| s.to_string()));
    let vf = match c.source {
        Source::GfxWindow | Source::GfxMonitor => {
            let target = if c.source == Source::GfxWindow {
                format!("hwnd={}", c.hwnd)
            } else {
                format!("hmonitor={}", c.monitor.hmon)
            };
            let input = format!(
                "gfxcapture={target}:max_framerate={fps}:width=-2:height=-2:resize_mode=scale_aspect:capture_cursor=1"
            );
            push(&["-f", "lavfi", "-i", &input]);
            format!("hwdownload,format=bgra,{scale},format=nv12")
        }
        Source::Ddagrab => {
            let input = format!("ddagrab=output_idx={}:framerate={fps}:draw_mouse=1", c.monitor_index);
            push(&["-f", "lavfi", "-i", &input]);
            format!("hwdownload,format=bgra,{scale},format=nv12")
        }
        Source::GdiWindow => {
            let input = format!("hwnd={}", c.hwnd);
            push(&["-f", "gdigrab", "-framerate", &fps, "-draw_mouse", "1", "-i", &input]);
            format!("crop=trunc(iw/2)*2:trunc(ih/2)*2,{scale},format=nv12")
        }
        Source::GdiDesktop => {
            let m = c.monitor;
            let (x, y, size) = (m.x.to_string(), m.y.to_string(), format!("{}x{}", m.w, m.h));
            push(&["-f", "gdigrab", "-framerate", &fps, "-draw_mouse", "1", "-offset_x", &x, "-offset_y", &y]);
            push(&["-video_size", &size, "-i", "desktop"]);
            format!("{scale},format=nv12")
        }
    };
    push(&["-vf", &vf, "-fps_mode", "cfr", "-r", &fps]);
    a.extend(encoder_args(c.encoder, c.crf, c.fps));
    a.extend(
        ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"]
            .map(String::from),
    );
    a.push(c.out.to_string_lossy().into_owned());
    a
}

#[derive(Default)]
struct Progress {
    frames: AtomicU64,
    fps_bits: AtomicU32,
    /// estimativa (relogio do app) do instante do primeiro frame
    start: Mutex<Option<f64>>,
}

pub struct VideoRecorder {
    child: Child,
    stdin: Option<ChildStdin>,
    progress: Arc<Progress>,
    stderr: Arc<Mutex<String>>,
    threads: Vec<JoinHandle<()>>,
}

impl VideoRecorder {
    pub fn start(c: &Capture) -> std::io::Result<Self> {
        let args = video_args(c);
        crate::log!("ffmpeg {}", args.join(" "));
        let mut child =
            ffmpeg().args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let mut stderr_pipe = child.stderr.take().expect("stderr");
        let progress = Arc::new(Progress::default());
        let stderr = Arc::new(Mutex::new(String::new()));
        let p = progress.clone();
        let reader = std::thread::spawn(move || read_progress(stdout, &p));
        let e = stderr.clone();
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr_pipe.read_to_string(&mut s);
            *e.lock().unwrap() = s;
        });
        Ok(Self { child, stdin, progress, stderr, threads: vec![reader, err_reader] })
    }

    pub fn frames(&self) -> u64 {
        self.progress.frames.load(Ordering::Relaxed)
    }

    pub fn fps(&self) -> f32 {
        f32::from_bits(self.progress.fps_bits.load(Ordering::Relaxed))
    }

    pub fn start_time(&self) -> Option<f64> {
        *self.progress.start.lock().unwrap()
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Saida de erro completa do FFmpeg (valida depois de `stop`/`kill`).
    pub fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }

    /// Encerra na hora (tentativa de captura que nao iniciou).
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stdin.take();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }

    /// Pede para o FFmpeg finalizar o arquivo ("q") e espera.
    pub fn stop(&mut self) {
        if self.alive() {
            if let Some(stdin) = self.stdin.as_mut() {
                let _ = stdin.write_all(b"q").and_then(|_| stdin.flush());
            }
            wait_timeout(&mut self.child, Duration::from_secs(15));
        }
        self.stdin.take();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for VideoRecorder {
    fn drop(&mut self) {
        if self.alive() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn read_progress(stdout: impl Read, p: &Progress) {
    let (mut frame, mut out_us, mut fps) = (0u64, 0i64, 0f32);
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let Some((k, v)) = line.trim().split_once('=') else { continue };
        match k {
            "frame" => frame = v.parse().unwrap_or(0),
            "out_time_us" => out_us = v.parse().unwrap_or(0),
            "fps" => fps = v.parse().unwrap_or(0.0),
            "progress" => {
                if frame > 0 && out_us > 0 {
                    // o menor valor observado e o mais proximo do real (menos atraso de buffer)
                    let est = clock::now() - out_us as f64 / 1e6;
                    let mut start = p.start.lock().unwrap();
                    if start.is_none_or(|s| est < s) {
                        *start = Some(est);
                    }
                }
                p.frames.store(frame, Ordering::Relaxed);
                p.fps_bits.store(fps.to_bits(), Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

pub struct AudioTrack {
    pub wav: PathBuf,
    /// segundos: positivo = audio comecou antes do video
    pub offset: f64,
    pub volume: f32,
}

/// Junta video + audios alinhados num MP4 (H.264 copiado + AAC).
pub fn mux(video: &Path, audios: &[AudioTrack], out: &Path) -> Result<(), String> {
    let mut cmd = ffmpeg();
    cmd.args(["-loglevel", "error", "-y", "-i"]).arg(video);
    let mut filters = Vec::new();
    for (i, a) in audios.iter().enumerate() {
        let i = i + 1;
        cmd.arg("-i").arg(&a.wav);
        let mut chain = format!("[{i}:a]");
        if a.offset > 0.0 {
            chain += &format!("atrim=start={:.4},asetpts=PTS-STARTPTS,", a.offset);
        } else if a.offset < 0.0 {
            chain += &format!("adelay={}:all=1,", (-a.offset * 1000.0) as i64);
        }
        chain += &format!("aresample=48000,aformat=channel_layouts=stereo,volume={:.2}[a{i}]", a.volume / 100.0);
        filters.push(chain);
    }
    match audios.len() {
        0 => {
            cmd.args(["-map", "0:v"]);
        }
        n => {
            let out_label = if n == 1 {
                "[a1]".to_string()
            } else {
                let labels: String = (1..=n).map(|i| format!("[a{i}]")).collect();
                filters.push(format!("{labels}amix=inputs={n}:duration=longest:normalize=0[aout]"));
                "[aout]".to_string()
            };
            cmd.args(["-filter_complex", &filters.join(";"), "-map", "0:v", "-map", &out_label]);
            cmd.args(["-c:a", "aac", "-b:a", "192k", "-shortest"]);
        }
    }
    cmd.args(["-c:v", "copy", "-movflags", "+faststart"]).arg(out);
    crate::log!("mux: {:?}", cmd.get_args().collect::<Vec<_>>());
    let r = cmd.stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if r.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&r.stderr);
        crate::log!("mux falhou ({}):\n{err}", r.status);
        let line = summarize_error(&err);
        Err(if line.is_empty() { "falha no ffmpeg".into() } else { line })
    }
}
