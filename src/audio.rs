//! Captura WASAPI (loopback do sistema e microfone) gravando WAV 16 bits.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::clock;

pub struct Devices {
    pub output: Option<Device>,
    pub mics: Vec<(String, Device)>,
}

/// Saida padrao (usada em loopback) e microfones, com o padrao primeiro.
pub fn devices() -> Devices {
    let host = cpal::default_host();
    let output = host.default_output_device();
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    let mut mics: Vec<(String, Device)> =
        host.input_devices().map(|it| it.filter_map(|d| d.name().ok().map(|n| (n, d))).collect()).unwrap_or_default();
    mics.sort_by_key(|(n, _)| Some(n) != default_in.as_ref());
    Devices { output, mics }
}

pub struct AudioRecorder {
    stream: Option<Stream>,
    keepalive: Option<Stream>,
    writer: Option<JoinHandle<()>>,
    start: Arc<OnceLock<f64>>,
    pub path: PathBuf,
}

impl AudioRecorder {
    /// Microfone (ou qualquer entrada).
    pub fn input(dev: &Device, path: &Path, level: Arc<AtomicU32>) -> Result<Self, String> {
        let sc = dev.default_input_config().map_err(|e| e.to_string())?;
        Self::start(dev, sc.sample_format(), sc.config(), path, level, None)
    }

    /// Loopback do que toca na saida. O WASAPI nao entrega dados em silencio,
    /// entao toca silencio na mesma saida para manter o fluxo continuo.
    pub fn loopback(dev: &Device, path: &Path, level: Arc<AtomicU32>) -> Result<Self, String> {
        let sc = dev.default_output_config().map_err(|e| e.to_string())?;
        let keepalive = match sc.sample_format() {
            SampleFormat::F32 => silence::<f32>(dev, &sc.config()),
            SampleFormat::I16 => silence::<i16>(dev, &sc.config()),
            SampleFormat::I32 => silence::<i32>(dev, &sc.config()),
            SampleFormat::U16 => silence::<u16>(dev, &sc.config()),
            _ => None,
        };
        Self::start(dev, sc.sample_format(), sc.config(), path, level, keepalive)
    }

    fn start(
        dev: &Device,
        fmt: SampleFormat,
        cfg: StreamConfig,
        path: &Path,
        level: Arc<AtomicU32>,
        keepalive: Option<Stream>,
    ) -> Result<Self, String> {
        let channels = cfg.channels.min(2);
        let spec = hound::WavSpec {
            channels,
            sample_rate: cfg.sample_rate.0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        // o callback de audio so converte e enfileira; disco fica nesta thread
        let writer = std::thread::spawn(move || {
            for buf in rx {
                for s in buf {
                    if wav.write_sample(s).is_err() {
                        return;
                    }
                }
            }
            let _ = wav.finalize();
        });
        let start = Arc::new(OnceLock::new());
        let st = start.clone();
        let stream = match fmt {
            SampleFormat::F32 => capture::<f32>(dev, &cfg, tx, level, st),
            SampleFormat::I16 => capture::<i16>(dev, &cfg, tx, level, st),
            SampleFormat::I32 => capture::<i32>(dev, &cfg, tx, level, st),
            SampleFormat::U16 => capture::<u16>(dev, &cfg, tx, level, st),
            f => Err(format!("formato de áudio não suportado: {f}")),
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                let _ = writer.join();
                return Err(e);
            }
        };
        Ok(Self { stream: Some(stream), keepalive, writer: Some(writer), start, path: path.to_path_buf() })
    }

    /// Instante (relogio do app) do primeiro sample capturado.
    pub fn start_time(&self) -> Option<f64> {
        self.start.get().copied()
    }

    /// Para a captura e fecha o WAV.
    pub fn stop(&mut self) {
        // soltar o stream solta o callback (e o sender), encerrando o writer
        self.stream.take();
        self.keepalive.take();
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.stop();
    }
}

fn capture<T>(
    dev: &Device,
    cfg: &StreamConfig,
    tx: mpsc::Sender<Vec<i16>>,
    level: Arc<AtomicU32>,
    start: Arc<OnceLock<f64>>,
) -> Result<Stream, String>
where
    T: SizedSample,
    i16: FromSample<T>,
{
    let ch = cfg.channels.max(1) as usize;
    let keep = ch.min(2);
    let rate = cfg.sample_rate.0 as f64;
    let stream = dev
        .build_input_stream(
            cfg,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let frames = data.len() / ch;
                start.get_or_init(|| clock::now() - frames as f64 / rate);
                let mut buf = Vec::with_capacity(frames * keep);
                let mut peak = 0u16;
                for frame in data.chunks_exact(ch) {
                    for s in &frame[..keep] {
                        let v: i16 = s.to_sample();
                        peak = peak.max(v.unsigned_abs());
                        buf.push(v);
                    }
                }
                // floats positivos comparam igual aos seus bits: fetch_max guarda o pico
                level.fetch_max((peak as f32 / 32768.0).to_bits(), Ordering::Relaxed);
                let _ = tx.send(buf);
            },
            |e| eprintln!("audio: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn silence<T: SizedSample + Send + 'static>(dev: &Device, cfg: &StreamConfig) -> Option<Stream> {
    let stream = dev
        .build_output_stream(
            cfg,
            |data: &mut [T], _: &cpal::OutputCallbackInfo| data.fill(T::EQUILIBRIUM),
            |e| eprintln!("audio keepalive: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}
