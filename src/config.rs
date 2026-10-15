//! Configuracao persistida em %APPDATA%\GameRec\config.json (mesmo formato da versao Python).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const QUALITY: [(&str, u32); 4] = [("Máxima", 16), ("Alta", 20), ("Média", 24), ("Leve", 28)];
pub const RESOLUTIONS: [(&str, Option<u32>); 4] =
    [("Nativa", None), ("1440p", Some(1440)), ("1080p", Some(1080)), ("720p", Some(720))];
pub const FPS: [&str; 2] = ["30", "60"];

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub output_dir: String,
    /// "auto:None", "mon:<indice>" ou "win:<exe>"
    pub source: String,
    pub fps: String,
    pub quality: String,
    pub resolution: String,
    pub system_audio: bool,
    pub system_volume: f32,
    pub mic: bool,
    pub mic_device: String,
    pub mic_volume: f32,
    pub minimize: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir().to_string_lossy().into_owned(),
            source: "auto:None".into(),
            fps: "60".into(),
            quality: "Alta".into(),
            resolution: "Nativa".into(),
            system_audio: true,
            system_volume: 100.0,
            mic: false,
            mic_device: String::new(),
            mic_volume: 100.0,
            minimize: false,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default()
}

fn default_output_dir() -> PathBuf {
    home().join("Videos").join("GameRec")
}

fn config_file() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("GameRec").join("config.json")
}

impl Config {
    pub fn load() -> Self {
        let mut cfg: Config =
            std::fs::read_to_string(config_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let def = Config::default();
        if !FPS.contains(&cfg.fps.as_str()) {
            cfg.fps = def.fps;
        }
        if !QUALITY.iter().any(|(k, _)| *k == cfg.quality) {
            cfg.quality = def.quality;
        }
        if !RESOLUTIONS.iter().any(|(k, _)| *k == cfg.resolution) {
            cfg.resolution = def.resolution;
        }
        if cfg.output_dir.is_empty() {
            cfg.output_dir = def.output_dir;
        }
        cfg.system_volume = cfg.system_volume.clamp(0.0, 200.0);
        cfg.mic_volume = cfg.mic_volume.clamp(0.0, 200.0);
        cfg
    }

    pub fn save(&self) {
        let path = config_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn crf(&self) -> u32 {
        QUALITY.iter().find(|(k, _)| *k == self.quality).map_or(20, |(_, q)| *q)
    }

    pub fn max_height(&self) -> Option<u32> {
        RESOLUTIONS.iter().find(|(k, _)| *k == self.resolution).and_then(|(_, h)| *h)
    }

    pub fn fps(&self) -> u32 {
        self.fps.parse().unwrap_or(60)
    }
}
