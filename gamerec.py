"""GameRec - gravador de tela com audio para gameplays (Windows)."""
import ctypes
import ctypes.wintypes as wt
import json
import os
import queue
import shutil
import subprocess
import sys
import threading
import time
import wave
from datetime import datetime
from pathlib import Path

import customtkinter as ctk
import pyaudiowpatch as pyaudio
from PIL import Image

APP_NAME = "GameRec"
CREATE_NO_WINDOW = 0x08000000

if getattr(sys, "frozen", False):
    BASE = Path(sys._MEIPASS)
else:
    BASE = Path(__file__).parent
FFMPEG = str(BASE / "bin" / "ffmpeg.exe")
ASSETS = BASE / "assets"

CONFIG_DIR = Path(os.environ.get("APPDATA", Path.home())) / APP_NAME
CONFIG_FILE = CONFIG_DIR / "config.json"
DEFAULT_OUT = Path.home() / "Videos" / APP_NAME

# Cores
BG = "#0f1117"
CARD = "#181b24"
CARD2 = "#1f2330"
ACCENT = "#ff3b5c"
ACCENT_HOVER = "#ff5c78"
TEXT = "#e8eaf0"
MUTED = "#8b90a0"
GREEN = "#2ecc71"

QUALITY = {"Máxima": 16, "Alta": 20, "Média": 24, "Leve": 28}
AUTO_LABEL = "Automático (jogo aberto)"
RESOLUTIONS = {"Nativa": None, "1440p": 1440, "1080p": 1080, "720p": 720}
ENCODER_NAMES = {
    "h264_nvenc": "NVIDIA NVENC",
    "h264_amf": "AMD AMF",
    "h264_qsv": "Intel Quick Sync",
    "libx264": "CPU (x264)",
}

DEFAULTS = {
    "output_dir": str(DEFAULT_OUT),
    "source": "auto:None",
    "fps": "60",
    "quality": "Alta",
    "resolution": "Nativa",
    "system_audio": True,
    "system_volume": 100,
    "mic": False,
    "mic_device": "",
    "mic_volume": 100,
    "minimize": False,
}


def run_hidden(cmd, **kw):
    return subprocess.run(cmd, creationflags=CREATE_NO_WINDOW, capture_output=True, **kw)


# ---------------------------------------------------------------- monitores
class RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                ("right", ctypes.c_long), ("bottom", ctypes.c_long)]


class MONITORINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.DWORD), ("rcMonitor", RECT),
                ("rcWork", RECT), ("dwFlags", wt.DWORD)]


def list_monitors():
    user32 = ctypes.windll.user32
    mons = []
    proc_t = ctypes.WINFUNCTYPE(ctypes.c_int, wt.HMONITOR, wt.HDC, ctypes.POINTER(RECT), wt.LPARAM)

    def cb(hmon, hdc, rect, lparam):
        info = MONITORINFO()
        info.cbSize = ctypes.sizeof(MONITORINFO)
        user32.GetMonitorInfoW(hmon, ctypes.byref(info))
        r = info.rcMonitor
        mons.append({"x": r.left, "y": r.top, "w": r.right - r.left, "h": r.bottom - r.top,
                     "primary": bool(info.dwFlags & 1), "hmon": hmon or 0})
        return 1

    user32.EnumDisplayMonitors(None, None, proc_t(cb), 0)
    mons.sort(key=lambda m: not m["primary"])
    return mons or [{"x": 0, "y": 0, "w": 1920, "h": 1080, "primary": True, "hmon": 0}]


def com_init():
    """WASAPI/PortAudio exige COM inicializado na thread que abre os streams."""
    try:
        ctypes.windll.ole32.CoInitializeEx(None, 0)
    except Exception:
        pass


# ---------------------------------------------------------------- janelas / jogos
# processos que nunca sao jogos
NOT_GAMES = {
    "explorer.exe", "steam.exe", "steamwebhelper.exe", "windowsterminal.exe", "applicationframehost.exe",
    "systemsettings.exe", "textinputhost.exe", "python.exe", "pythonw.exe", "gamerec.exe",
    "shellexperiencehost.exe", "searchhost.exe", "startmenuexperiencehost.exe", "lockapp.exe",
    "epicgameslauncher.exe", "riotclientux.exe", "riotclientservices.exe", "battle.net.exe",
    "galaxyclient.exe", "eadesktop.exe", "ubisoftconnect.exe", "upc.exe", "discord.exe",
    "obs64.exe", "taskmgr.exe", "code.exe", "nvidia app.exe", "nvcontainer.exe",
    "cmd.exe", "powershell.exe", "conhost.exe", "notepad.exe", "mmc.exe",
}
BROWSERS = {"chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "brave.exe", "spotify.exe",
            "whatsapp.exe", "teams.exe", "ms-teams.exe", "slack.exe", "telegram.exe"}
GAME_PATHS = ("steamapps", "epic games", "riot games", "gog galaxy", "xboxgames", "\\games\\",
              "ubisoft game launcher\\games", "ea games", "battle.net", "rockstar games", "minecraft")


def _exe_path(pid):
    k32 = ctypes.windll.kernel32
    h = k32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not h:
        return ""
    try:
        buf = ctypes.create_unicode_buffer(1024)
        size = wt.DWORD(1024)
        if k32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(size)):
            return buf.value
        return ""
    finally:
        k32.CloseHandle(h)


def list_windows(monitors):
    """Janelas visiveis de aplicativos, com pontuacao de 'parece jogo'."""
    user32 = ctypes.windll.user32
    dwm = ctypes.windll.dwmapi
    own_pid = os.getpid()
    wins = []
    proc_t = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)

    def cb(hwnd, lparam):
        if not user32.IsWindowVisible(hwnd) or user32.GetWindow(hwnd, 4):  # GW_OWNER
            return True
        if user32.GetWindowLongW(hwnd, -20) & 0x80:  # WS_EX_TOOLWINDOW
            return True
        cloaked = ctypes.c_int(0)
        dwm.DwmGetWindowAttribute(hwnd, 14, ctypes.byref(cloaked), 4)
        if cloaked.value:
            return True
        n = user32.GetWindowTextLengthW(hwnd)
        if n == 0:
            return True
        title = ctypes.create_unicode_buffer(n + 1)
        user32.GetWindowTextW(hwnd, title, n + 1)
        pid = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == own_pid:
            return True
        path = _exe_path(pid.value)
        exe = os.path.basename(path).lower()
        if exe in NOT_GAMES or not exe:
            return True
        r = RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(r))
        iconic = bool(user32.IsIconic(hwnd))
        score = 0
        low = path.lower()
        if any(p in low for p in GAME_PATHS):
            score += 3
        if exe in BROWSERS:
            score -= 5
        if not iconic and any(r.left <= m["x"] and r.top <= m["y"] and r.right >= m["x"] + m["w"]
                              and r.bottom >= m["y"] + m["h"] for m in monitors):
            score += 2  # tela cheia
        wins.append({"hwnd": hwnd or 0, "title": title.value, "exe": exe, "score": score,
                     "iconic": iconic})
        return True

    user32.EnumWindows(proc_t(cb), 0)
    wins.sort(key=lambda w: -w["score"])
    return wins


def foreground_hwnd():
    return ctypes.windll.user32.GetForegroundWindow() or 0


# ---------------------------------------------------------------- hotkey global
class GlobalHotkey(threading.Thread):
    WM_HOTKEY = 0x0312
    MOD_NOREPEAT = 0x4000

    def __init__(self, vk, callback):
        super().__init__(daemon=True)
        self.vk = vk
        self.callback = callback
        self.ok = False

    def run(self):
        user32 = ctypes.windll.user32
        self.ok = bool(user32.RegisterHotKey(None, 1, self.MOD_NOREPEAT, self.vk))
        msg = wt.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) != 0:
            if msg.message == self.WM_HOTKEY:
                self.callback()


# ---------------------------------------------------------------- audio
class AudioRecorder:
    """Grava um dispositivo WASAPI (microfone ou loopback do sistema) em WAV."""

    def __init__(self, pa, device, path, keepalive_output=None):
        self.pa = pa
        self.device = device
        self.path = path
        self.channels = max(1, min(2, int(device["maxInputChannels"])))
        self.rate = int(device["defaultSampleRate"])
        self.q = queue.Queue()
        self.start_time = None
        self.level = 0.0
        self.keepalive_output = keepalive_output
        self._ka_stream = None
        self._stream = None
        self._writer = None

    def _callback(self, data, frames, time_info, status):
        if self.start_time is None:
            self.start_time = time.perf_counter() - frames / self.rate
        self.q.put(data)
        # nivel aproximado (pico) para o medidor
        try:
            import array
            a = array.array("h", data)
            if a:
                self.level = min(1.0, max(abs(min(a)), max(a)) / 32768)
        except Exception:
            pass
        return (None, pyaudio.paContinue)

    def _write_loop(self):
        with wave.open(str(self.path), "wb") as wf:
            wf.setnchannels(self.channels)
            wf.setsampwidth(2)
            wf.setframerate(self.rate)
            while True:
                chunk = self.q.get()
                if chunk is None:
                    break
                wf.writeframes(chunk)

    def start(self):
        if self.keepalive_output is not None:
            # Loopback WASAPI nao entrega dados em silencio; tocar silencio
            # mantem o fluxo continuo e o audio sincronizado.
            out = self.keepalive_output
            ch = max(1, int(out["maxOutputChannels"]))
            rate = int(out["defaultSampleRate"])
            silence_cache = {}

            def ka_cb(data, frames, ti, st):
                buf = silence_cache.get(frames)
                if buf is None:
                    buf = silence_cache[frames] = bytes(frames * ch * 2)
                return (buf, pyaudio.paContinue)

            try:
                self._ka_stream = self.pa.open(format=pyaudio.paInt16, channels=ch, rate=rate,
                                               output=True, output_device_index=out["index"],
                                               stream_callback=ka_cb)
            except Exception:
                self._ka_stream = None
        self._writer = threading.Thread(target=self._write_loop, daemon=True)
        self._writer.start()
        try:
            self._stream = self.pa.open(format=pyaudio.paInt16, channels=self.channels, rate=self.rate,
                                        input=True, input_device_index=self.device["index"],
                                        frames_per_buffer=1024, stream_callback=self._callback)
        except Exception:
            self.stop()
            raise

    def stop(self):
        for s in (self._stream, self._ka_stream):
            if s is not None:
                try:
                    s.stop_stream()
                    s.close()
                except Exception:
                    pass
        self.q.put(None)
        if self._writer:
            self._writer.join(timeout=10)


def wasapi_devices(pa):
    """Retorna (loopback_padrao, saida_padrao, [microfones])."""
    try:
        wasapi = pa.get_host_api_info_by_type(pyaudio.paWASAPI)
    except OSError:
        return None, None, []
    out = None
    loopback = None
    try:
        out = pa.get_device_info_by_index(wasapi["defaultOutputDevice"])
        if out.get("isLoopbackDevice"):
            loopback = out
        else:
            for lb in pa.get_loopback_device_info_generator():
                if out["name"] in lb["name"]:
                    loopback = lb
                    break
    except Exception:
        pass
    mics = []
    default_in = wasapi.get("defaultInputDevice", -1)
    for i in range(pa.get_device_count()):
        d = pa.get_device_info_by_index(i)
        if d["hostApi"] == wasapi["index"] and d["maxInputChannels"] > 0 and not d.get("isLoopbackDevice"):
            d = dict(d)
            d["is_default"] = i == default_in
            mics.append(d)
    mics.sort(key=lambda d: not d["is_default"])
    return loopback, out, mics


# ---------------------------------------------------------------- video
def detect_encoder():
    for enc in ("h264_nvenc", "h264_amf", "h264_qsv", "libx264"):
        cmd = [FFMPEG, "-hide_banner", "-loglevel", "error", "-f", "lavfi",
               "-i", "color=black:s=1280x720:r=30", "-frames:v", "10",
               "-pix_fmt", "nv12", "-c:v", enc, "-f", "null", "-"]
        try:
            if run_hidden(cmd, timeout=20).returncode == 0:
                return enc
        except Exception:
            pass
    return "libx264"


def encoder_args(enc, q, fps):
    gop = str(int(fps) * 2)
    if enc == "h264_nvenc":
        return ["-c:v", enc, "-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", str(q),
                "-b:v", "0", "-maxrate", "100M", "-bufsize", "200M", "-profile:v", "high", "-g", gop]
    if enc == "h264_amf":
        return ["-c:v", enc, "-quality", "balanced", "-rc", "cqp", "-qp_i", str(q),
                "-qp_p", str(q + 2), "-profile:v", "high", "-g", gop]
    if enc == "h264_qsv":
        return ["-c:v", enc, "-preset", "medium", "-global_quality", str(q),
                "-profile:v", "high", "-g", gop]
    return ["-c:v", "libx264", "-preset", "veryfast", "-crf", str(q),
            "-profile:v", "high", "-g", gop]


def build_video_cmd(source, enc, mon_idx, mon, fps, res_h, q, out, win=None):
    """source: gfx_window | gfx_monitor | ddagrab | gdigrab_window | gdigrab."""
    cmd = [FFMPEG, "-hide_banner", "-loglevel", "error", "-nostats", "-y",
           "-progress", "pipe:1", "-stats_period", "0.1"]
    scale = "scale=out_color_matrix=bt709:out_range=tv"
    if res_h:
        scale = f"scale=-2:'min(ih,{res_h})':flags=bicubic:out_color_matrix=bt709:out_range=tv"
    if source.startswith("gfx"):
        target = f"hwnd={win['hwnd']}" if source == "gfx_window" else f"hmonitor={mon['hmon']}"
        cmd += ["-f", "lavfi", "-i", f"gfxcapture={target}:max_framerate={fps}:width=-2:height=-2"
                                     f":resize_mode=scale_aspect:capture_cursor=1"]
        vf = f"hwdownload,format=bgra,{scale},format=nv12"
    elif source == "ddagrab":
        cmd += ["-f", "lavfi", "-i", f"ddagrab=output_idx={mon_idx}:framerate={fps}:draw_mouse=1"]
        vf = f"hwdownload,format=bgra,{scale},format=nv12"
    elif source == "gdigrab_window":
        cmd += ["-f", "gdigrab", "-framerate", str(fps), "-draw_mouse", "1",
                "-i", f"hwnd={win['hwnd']}"]
        vf = f"crop=trunc(iw/2)*2:trunc(ih/2)*2,{scale},format=nv12"
    else:
        cmd += ["-f", "gdigrab", "-framerate", str(fps), "-draw_mouse", "1",
                "-offset_x", str(mon["x"]), "-offset_y", str(mon["y"]),
                "-video_size", f"{mon['w']}x{mon['h']}", "-i", "desktop"]
        vf = f"{scale},format=nv12"
    cmd += ["-vf", vf, "-fps_mode", "cfr", "-r", str(fps)]
    cmd += encoder_args(enc, q, fps)
    cmd += ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
            "-color_range", "tv", out]
    return cmd


class VideoRecorder:
    def __init__(self, cmd):
        self.cmd = cmd
        self.proc = None
        self.start_time = None
        self.frames = 0
        self.fps_now = 0.0
        self._reader = None

    def start(self):
        self.proc = subprocess.Popen(self.cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
        self._reader = threading.Thread(target=self._read_progress, daemon=True)
        self._reader.start()

    def _read_progress(self):
        block = {}
        for raw in self.proc.stdout:
            line = raw.decode(errors="ignore").strip()
            if "=" not in line:
                continue
            k, v = line.split("=", 1)
            block[k] = v
            if k == "progress":
                now = time.perf_counter()
                try:
                    frames = int(block.get("frame", "0"))
                    out_us = int(block.get("out_time_us", "0"))
                    self.fps_now = float(block.get("fps", "0") or 0)
                except ValueError:
                    frames, out_us = 0, 0
                if frames > 0 and out_us > 0:
                    # estimativa do instante do primeiro frame; o menor valor
                    # observado e o mais proximo do real (menos atraso de buffer)
                    est = now - out_us / 1e6
                    if self.start_time is None or est < self.start_time:
                        self.start_time = est
                self.frames = frames
                block = {}

    def alive(self):
        return self.proc is not None and self.proc.poll() is None

    def stderr_text(self):
        try:
            return self.proc.stderr.read().decode(errors="ignore")[-600:]
        except Exception:
            return ""

    def stop(self):
        if self.alive():
            try:
                self.proc.stdin.write(b"q")
                self.proc.stdin.flush()
            except Exception:
                pass
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        if self._reader:
            self._reader.join(timeout=3)


def mux(video, audios, out):
    """audios: lista de (wav_path, start_time, volume%)  ; video_start em audios[i][3]."""
    cmd = [FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-i", str(video)]
    filters = []
    labels = []
    for i, (wav, offset, vol) in enumerate(audios, start=1):
        cmd += ["-i", str(wav)]
        chain = f"[{i}:a]"
        if offset > 0:  # audio comecou antes do video: corta o inicio
            chain += f"atrim=start={offset:.4f},asetpts=PTS-STARTPTS,"
        elif offset < 0:  # audio comecou depois: atrasa
            chain += f"adelay={int(-offset * 1000)}:all=1,"
        chain += f"aresample=48000,aformat=channel_layouts=stereo,volume={vol / 100:.2f}[a{i}]"
        filters.append(chain)
        labels.append(f"[a{i}]")
    if audios:
        if len(labels) > 1:
            filters.append("".join(labels) + f"amix=inputs={len(labels)}:duration=longest:normalize=0[aout]")
        else:
            filters[-1] = filters[-1].replace(f"[a{len(audios)}]", "[aout]")
        cmd += ["-filter_complex", ";".join(filters), "-map", "0:v", "-map", "[aout]",
                "-c:a", "aac", "-b:a", "192k", "-shortest"]
    else:
        cmd += ["-map", "0:v"]
    cmd += ["-c:v", "copy", "-movflags", "+faststart", str(out)]
    r = run_hidden(cmd)
    return r.returncode == 0, r.stderr.decode(errors="ignore")[-600:]


# ---------------------------------------------------------------- UI
def load_config():
    cfg = dict(DEFAULTS)
    try:
        cfg.update(json.loads(CONFIG_FILE.read_text(encoding="utf-8")))
    except Exception:
        pass
    return cfg


def save_config(cfg):
    try:
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        CONFIG_FILE.write_text(json.dumps(cfg, indent=2, ensure_ascii=False), encoding="utf-8")
    except Exception:
        pass


def fmt_time(s):
    s = int(s)
    return f"{s // 3600:02d}:{s % 3600 // 60:02d}:{s % 60:02d}"


class Card(ctk.CTkFrame):
    def __init__(self, master, title, **kw):
        super().__init__(master, fg_color=CARD, corner_radius=14, **kw)
        ctk.CTkLabel(self, text=title, font=ctk.CTkFont(size=13, weight="bold"),
                     text_color=MUTED).pack(anchor="w", padx=18, pady=(14, 6))


class App(ctk.CTk):
    def __init__(self):
        super().__init__()
        ctk.set_appearance_mode("dark")
        self.cfg = load_config()
        self.title(APP_NAME)
        self.geometry("900x720")
        self.minsize(820, 600)
        self.configure(fg_color=BG)
        try:
            self.iconbitmap(str(ASSETS / "icon.ico"))
        except Exception:
            pass

        self.pa = pyaudio.PyAudio()
        self.loopback, self.out_dev, self.mics = wasapi_devices(self.pa)
        self.monitors = list_monitors()
        self.encoder = None
        self.recording = False
        self.busy = False
        self.rec_start = 0.0
        self.rec_target = ""
        self.video = None
        self.audio_recs = []
        self.session = None

        self._build_ui()
        threading.Thread(target=self._detect_encoder, daemon=True).start()
        self.hotkey = GlobalHotkey(0x78, self._on_hotkey)  # F9
        self.hotkey.start()
        self.protocol("WM_DELETE_WINDOW", self.on_close)
        self._tick()
        self.after(3000, self._scan_loop)

    # ---------- layout
    def _build_ui(self):
        self.grid_columnconfigure(0, weight=3)
        self.grid_columnconfigure(1, weight=2)
        self.grid_rowconfigure(1, weight=1)

        header = ctk.CTkFrame(self, fg_color="transparent")
        header.grid(row=0, column=0, columnspan=2, sticky="ew", padx=24, pady=(20, 8))
        try:
            logo = ctk.CTkImage(Image.open(ASSETS / "icon.png"), size=(40, 40))
            ctk.CTkLabel(header, image=logo, text="").pack(side="left", padx=(0, 12))
        except Exception:
            pass
        tbox = ctk.CTkFrame(header, fg_color="transparent")
        tbox.pack(side="left")
        ctk.CTkLabel(tbox, text=APP_NAME, font=ctk.CTkFont(size=26, weight="bold"),
                     text_color=TEXT).pack(anchor="w")
        ctk.CTkLabel(tbox, text="Gravador de gameplay para YouTube", font=ctk.CTkFont(size=13),
                     text_color=MUTED).pack(anchor="w")
        self.enc_label = ctk.CTkLabel(header, text="  Detectando encoder...  ", corner_radius=10,
                                      fg_color=CARD2, text_color=MUTED, font=ctk.CTkFont(size=12))
        self.enc_label.pack(side="right", ipady=4)

        # ---- coluna esquerda: gravacao
        left = ctk.CTkFrame(self, fg_color="transparent")
        left.grid(row=1, column=0, sticky="nsew", padx=(24, 10), pady=(8, 20))
        left.grid_rowconfigure(1, weight=1)
        left.grid_columnconfigure(0, weight=1)

        rec = ctk.CTkFrame(left, fg_color=CARD, corner_radius=14)
        rec.grid(row=0, column=0, sticky="nsew")
        self.status_dot = ctk.CTkLabel(rec, text="●  PRONTO", text_color=MUTED,
                                       font=ctk.CTkFont(size=13, weight="bold"))
        self.status_dot.pack(pady=(22, 0))
        self.timer = ctk.CTkLabel(rec, text="00:00:00", text_color=TEXT,
                                  font=ctk.CTkFont(family="Consolas", size=58, weight="bold"))
        self.timer.pack(pady=(4, 0))
        self.info = ctk.CTkLabel(rec, text="Pressione F9 em qualquer lugar para iniciar/parar",
                                 text_color=MUTED, font=ctk.CTkFont(size=12))
        self.info.pack()
        self.rec_btn = ctk.CTkButton(rec, text="⏺   INICIAR GRAVAÇÃO", height=56, corner_radius=28,
                                     font=ctk.CTkFont(size=17, weight="bold"), fg_color=ACCENT,
                                     hover_color=ACCENT_HOVER, command=self.toggle)
        self.rec_btn.pack(fill="x", padx=40, pady=(18, 14))

        meters = ctk.CTkFrame(rec, fg_color="transparent")
        meters.pack(fill="x", padx=40, pady=(0, 22))
        meters.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(meters, text="Sistema", text_color=MUTED, width=70, anchor="w").grid(row=0, column=0)
        self.meter_sys = ctk.CTkProgressBar(meters, height=8, progress_color=GREEN, fg_color=CARD2)
        self.meter_sys.grid(row=0, column=1, sticky="ew", pady=4)
        ctk.CTkLabel(meters, text="Microfone", text_color=MUTED, width=70, anchor="w").grid(row=1, column=0)
        self.meter_mic = ctk.CTkProgressBar(meters, height=8, progress_color=GREEN, fg_color=CARD2)
        self.meter_mic.grid(row=1, column=1, sticky="ew", pady=4)
        self.meter_sys.set(0)
        self.meter_mic.set(0)

        files = Card(left, "GRAVAÇÕES RECENTES")
        files.grid(row=1, column=0, sticky="nsew", pady=(14, 0))
        self.files_box = ctk.CTkScrollableFrame(files, fg_color="transparent", height=120)
        self.files_box.pack(fill="both", expand=True, padx=10, pady=(0, 6))
        bar = ctk.CTkFrame(files, fg_color="transparent")
        bar.pack(fill="x", padx=18, pady=(0, 14))
        ctk.CTkButton(bar, text="Abrir pasta", width=110, fg_color=CARD2, hover_color="#2a3042",
                      command=self.open_folder).pack(side="left")
        ctk.CTkButton(bar, text="Abrir YouTube Studio", width=160, fg_color=CARD2,
                      hover_color="#2a3042", command=lambda: os.startfile(
                          "https://studio.youtube.com")).pack(side="left", padx=8)

        # ---- coluna direita: configuracoes
        right = ctk.CTkScrollableFrame(self, fg_color="transparent")
        right.grid(row=1, column=1, sticky="nsew", padx=(10, 24), pady=(8, 20))

        vid = Card(right, "VÍDEO")
        vid.pack(fill="x")
        self.src_var = ctk.StringVar()
        row = self._row(vid, "Gravar")
        self.src_menu = ctk.CTkOptionMenu(row, variable=self.src_var, values=[AUTO_LABEL],
                                          fg_color=CARD2, button_color=CARD2,
                                          button_hover_color="#2a3042", dynamic_resizing=False)
        self.src_menu.pack(side="left", fill="x", expand=True)
        ctk.CTkButton(row, text="↻", width=30, fg_color=CARD2, hover_color="#2a3042",
                      command=self.refresh_sources).pack(side="left", padx=(6, 0))
        self.detect_label = ctk.CTkLabel(vid, text="", text_color=MUTED, anchor="w",
                                         font=ctk.CTkFont(size=12))
        self.detect_label.pack(fill="x", padx=18)
        self.refresh_sources(initial=True)
        self.fps_var = ctk.StringVar(value=self.cfg["fps"])
        self._segmented(vid, "FPS", self.fps_var, ["30", "60"])
        self.res_var = ctk.StringVar(value=self.cfg["resolution"])
        self._option(vid, "Resolução", self.res_var, list(RESOLUTIONS))
        self.q_var = ctk.StringVar(value=self.cfg["quality"])
        self._segmented(vid, "Qualidade", self.q_var, list(QUALITY))
        ctk.CTkFrame(vid, height=8, fg_color="transparent").pack()

        aud = Card(right, "ÁUDIO")
        aud.pack(fill="x", pady=(14, 0))
        self.sys_var = ctk.BooleanVar(value=self.cfg["system_audio"] and self.loopback is not None)
        sw = ctk.CTkSwitch(aud, text="Som do computador (jogo)", variable=self.sys_var,
                           progress_color=ACCENT)
        sw.pack(anchor="w", padx=18, pady=4)
        if self.loopback is None:
            sw.configure(state="disabled")
        self.sys_vol = self._slider(aud, self.cfg["system_volume"])
        self.mic_var = ctk.BooleanVar(value=self.cfg["mic"] and bool(self.mics))
        msw = ctk.CTkSwitch(aud, text="Microfone", variable=self.mic_var, progress_color=ACCENT)
        msw.pack(anchor="w", padx=18, pady=(10, 4))
        if not self.mics:
            msw.configure(state="disabled")
        self.mic_names = [m["name"] for m in self.mics] or ["Nenhum microfone encontrado"]
        mic_sel = self.cfg["mic_device"] if self.cfg["mic_device"] in self.mic_names else self.mic_names[0]
        self.mic_dev_var = ctk.StringVar(value=mic_sel)
        ctk.CTkOptionMenu(aud, variable=self.mic_dev_var, values=self.mic_names, fg_color=CARD2,
                          button_color=CARD2, button_hover_color="#2a3042",
                          dynamic_resizing=False).pack(fill="x", padx=18, pady=4)
        self.mic_vol = self._slider(aud, self.cfg["mic_volume"])
        ctk.CTkFrame(aud, height=8, fg_color="transparent").pack()

        out = Card(right, "SAÍDA")
        out.pack(fill="x", pady=(14, 0))
        self.out_label = ctk.CTkLabel(out, text=self._short(self.cfg["output_dir"]), text_color=TEXT,
                                      anchor="w")
        self.out_label.pack(fill="x", padx=18)
        ctk.CTkButton(out, text="Alterar pasta", fg_color=CARD2, hover_color="#2a3042",
                      command=self.choose_folder).pack(anchor="w", padx=18, pady=8)
        self.min_var = ctk.BooleanVar(value=self.cfg["minimize"])
        ctk.CTkSwitch(out, text="Minimizar ao gravar", variable=self.min_var,
                      progress_color=ACCENT).pack(anchor="w", padx=18, pady=(0, 16))

        self.refresh_files()

    def _row(self, parent, label):
        row = ctk.CTkFrame(parent, fg_color="transparent")
        row.pack(fill="x", padx=18, pady=5)
        ctk.CTkLabel(row, text=label, text_color=TEXT, width=80, anchor="w").pack(side="left")
        return row

    def _option(self, parent, label, var, values):
        row = self._row(parent, label)
        ctk.CTkOptionMenu(row, variable=var, values=values, fg_color=CARD2, button_color=CARD2,
                          button_hover_color="#2a3042", dynamic_resizing=False).pack(
            side="left", fill="x", expand=True)

    def _segmented(self, parent, label, var, values):
        row = self._row(parent, label)
        ctk.CTkSegmentedButton(row, variable=var, values=values, selected_color=ACCENT,
                               selected_hover_color=ACCENT_HOVER, fg_color=CARD2,
                               unselected_color=CARD2).pack(side="left", fill="x", expand=True)

    def _slider(self, parent, value):
        row = ctk.CTkFrame(parent, fg_color="transparent")
        row.pack(fill="x", padx=18, pady=2)
        lbl = ctk.CTkLabel(row, text=f"{int(value)}%", width=44, text_color=MUTED)
        s = ctk.CTkSlider(row, from_=0, to=200, number_of_steps=40, button_color=ACCENT,
                          button_hover_color=ACCENT_HOVER, progress_color=ACCENT,
                          command=lambda v: lbl.configure(text=f"{int(v)}%"))
        s.set(value)
        s.pack(side="left", fill="x", expand=True)
        lbl.pack(side="left", padx=(8, 0))
        return s

    @staticmethod
    def _short(p, n=38):
        return p if len(p) <= n else "..." + p[-(n - 3):]

    # ---------- acoes
    def _detect_encoder(self):
        enc = detect_encoder()
        self.encoder = enc
        color = GREEN if enc != "libx264" else MUTED
        self.after(0, lambda: self.enc_label.configure(
            text=f"  ⚡ {ENCODER_NAMES.get(enc, enc)}  ", text_color=color))

    def _on_hotkey(self):
        fg = foreground_hwnd()  # janela em foco no momento do F9 (o jogo)
        self.after(0, lambda: self.toggle(fg))

    # ---------- fontes de captura
    def refresh_sources(self, initial=False):
        """Atualiza lista de monitores/janelas e o jogo detectado."""
        self.monitors = list_monitors()
        self.windows = list_windows(self.monitors)
        self.sources = {AUTO_LABEL: ("auto", None)}
        for i, m in enumerate(self.monitors):
            label = f"Tela: Monitor {i + 1} - {m['w']}x{m['h']}" + (" (principal)" if m["primary"] else "")
            self.sources[label] = ("mon", i)
        seen = set()
        for w in self.windows:
            if w["exe"] in seen:
                continue
            seen.add(w["exe"])
            icon = "Jogo:" if w["score"] >= 2 else "Janela:"
            title = w["title"] if len(w["title"]) <= 34 else w["title"][:33] + "…"
            self.sources[f"{icon} {title}"] = ("win", w["exe"])
        self.src_menu.configure(values=list(self.sources))
        current = self.src_var.get()
        if initial or current not in self.sources:
            want = self.cfg.get("source", "auto")
            label = next((k for k, v in self.sources.items() if f"{v[0]}:{v[1]}" == want), None)
            if label is None and want.startswith("mon:"):
                label = next(k for k, v in self.sources.items() if v[0] == "mon")
            self.src_var.set(label or AUTO_LABEL)
        game = self.best_game()
        if game:
            txt = f"Jogo detectado: {game['title']}" + ("  (minimizado)" if game["iconic"] else "")
            self.detect_label.configure(text=txt, text_color=GREEN if not game["iconic"] else "#f5a623")
        else:
            self.detect_label.configure(text="Nenhum jogo detectado - Automático grava o monitor principal",
                                        text_color=MUTED)

    def best_game(self, fg=None):
        """Janela em foco (se parece app) > maior pontuacao de jogo."""
        if fg:
            w = next((w for w in self.windows if w["hwnd"] == fg), None)
            if w and w["score"] >= 0:
                return w
        return next((w for w in self.windows if w["score"] >= 2), None)

    def _scan_loop(self):
        if not self.recording and not self.busy:
            try:
                self.refresh_sources()
            except Exception:
                pass
        self.after(3000, self._scan_loop)

    def resolve_source(self, fg=None):
        """Retorna ('win', janela) ou ('mon', indice)."""
        kind, val = self.sources.get(self.src_var.get(), ("auto", None))
        if kind == "mon":
            return "mon", val
        self.windows = list_windows(self.monitors)
        if kind == "win":
            w = next((w for w in self.windows if w["exe"] == val), None)
            return ("win", w) if w else ("missing", val)
        game = self.best_game(fg)
        return ("win", game) if game else ("mon", 0)

    def collect_cfg(self):
        kind, val = self.sources.get(self.src_var.get(), ("auto", None))
        self.cfg.update({
            "source": f"{kind}:{val}",
            "fps": self.fps_var.get(),
            "quality": self.q_var.get(),
            "resolution": self.res_var.get(),
            "system_audio": self.sys_var.get(),
            "system_volume": int(self.sys_vol.get()),
            "mic": self.mic_var.get(),
            "mic_device": self.mic_dev_var.get(),
            "mic_volume": int(self.mic_vol.get()),
            "minimize": self.min_var.get(),
        })
        save_config(self.cfg)

    def choose_folder(self):
        d = ctk.filedialog.askdirectory(initialdir=self.cfg["output_dir"])
        if d:
            self.cfg["output_dir"] = os.path.normpath(d)
            self.out_label.configure(text=self._short(self.cfg["output_dir"]))
            save_config(self.cfg)
            self.refresh_files()

    def open_folder(self):
        p = Path(self.cfg["output_dir"])
        p.mkdir(parents=True, exist_ok=True)
        os.startfile(p)

    def refresh_files(self):
        for w in self.files_box.winfo_children():
            w.destroy()
        p = Path(self.cfg["output_dir"])
        vids = sorted(p.glob("*.mp4"), key=lambda f: f.stat().st_mtime, reverse=True)[:8] if p.exists() else []
        if not vids:
            ctk.CTkLabel(self.files_box, text="Nenhuma gravação ainda", text_color=MUTED).pack(pady=10)
        for f in vids:
            row = ctk.CTkFrame(self.files_box, fg_color=CARD2, corner_radius=8)
            row.pack(fill="x", pady=3)
            size = f.stat().st_size / 1024 / 1024
            ctk.CTkLabel(row, text=f"  {f.name}", text_color=TEXT, anchor="w").pack(
                side="left", padx=10, pady=6)
            ctk.CTkButton(row, text="▶", width=34, height=26, fg_color=ACCENT, hover_color=ACCENT_HOVER,
                          command=lambda f=f: os.startfile(f)).pack(side="right", padx=6)
            ctk.CTkLabel(row, text=f"{size:.0f} MB", text_color=MUTED).pack(side="right", padx=4)

    def toggle(self, fg=None):
        if self.busy:
            return
        if self.recording:
            self.stop_recording()
        else:
            self.start_recording(fg)

    def set_status(self, text, color):
        self.status_dot.configure(text=text, text_color=color)

    def start_recording(self, fg=None):
        if self.encoder is None:
            self.info.configure(text="Aguarde a detecção do encoder...")
            return
        self.collect_cfg()
        kind, target = self.resolve_source(fg)
        if kind == "missing":
            self.set_status("●  JANELA NÃO ENCONTRADA", ACCENT)
            self.info.configure(text=f"{target} não está aberto. Abra o jogo ou escolha outra fonte.")
            return
        win = target if kind == "win" else None
        if win and win["iconic"]:
            self.set_status("●  JOGO MINIMIZADO", "#f5a623")
            self.info.configure(text=f"Abra \"{win['title']}\" e aperte F9 dentro do jogo para gravar.")
            return
        mon_idx = target if kind == "mon" else 0
        mon = self.monitors[mon_idx]
        sources = ("gfx_window", "gdigrab_window") if win else ("gfx_monitor", "ddagrab", "gdigrab")
        self.rec_target = win["title"] if win else f"Monitor {mon_idx + 1}"
        out_dir = Path(self.cfg["output_dir"])
        out_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
        tmp = out_dir / f".gamerec_{stamp}"
        tmp.mkdir(exist_ok=True)
        self.session = {"tmp": tmp, "final": out_dir / f"Gameplay_{stamp}.mp4"}

        fps = int(self.cfg["fps"])
        q = QUALITY[self.cfg["quality"]]
        res_h = RESOLUTIONS[self.cfg["resolution"]]
        video_path = str(tmp / "video.mkv")
        self.session["video"] = video_path

        self.busy = True
        self.rec_btn.configure(state="disabled", text="Iniciando...")

        def worker():
            # audio primeiro (o video e alinhado depois via timestamps)
            com_init()
            recs = []
            err = None
            try:
                if self.cfg["system_audio"] and self.loopback:
                    r = AudioRecorder(self.pa, self.loopback, tmp / "system.wav", keepalive_output=self.out_dev)
                    r.start()
                    recs.append(("sys", r, self.cfg["system_volume"]))
                if self.cfg["mic"] and self.mics:
                    dev = next((m for m in self.mics if m["name"] == self.cfg["mic_device"]), self.mics[0])
                    r = AudioRecorder(self.pa, dev, tmp / "mic.wav")
                    r.start()
                    recs.append(("mic", r, self.cfg["mic_volume"]))
            except Exception as e:
                err = f"Erro de áudio: {e}"
            video = None
            if err is None:
                for source in sources:
                    for enc in dict.fromkeys((self.encoder, "libx264")):
                        cmd = build_video_cmd(source, enc, mon_idx, mon, fps, res_h, q, video_path, win)
                        v = VideoRecorder(cmd)
                        v.start()
                        t = time.perf_counter()
                        while time.perf_counter() - t < 4 and v.alive() and v.frames == 0:
                            time.sleep(0.05)
                        if v.alive() and v.frames > 0:
                            video = v
                            break
                        v.stop()
                        err = v.stderr_text().strip().splitlines()[-1:] or ["falha ao iniciar captura"]
                        err = "Erro de vídeo: " + err[0]
                    if video:
                        err = None
                        break
            if video is None:
                for _, r, _ in recs:
                    r.stop()
                shutil.rmtree(tmp, ignore_errors=True)
            self.after(0, lambda: self._started(video, recs, err))

        threading.Thread(target=worker, daemon=True).start()

    def _started(self, video, recs, err):
        self.busy = False
        if video is None:
            self.rec_btn.configure(state="normal", text="⏺   INICIAR GRAVAÇÃO")
            self.set_status("●  ERRO", ACCENT)
            self.info.configure(text=err or "Falha ao iniciar")
            return
        self.video = video
        self.audio_recs = recs
        self.recording = True
        self.rec_start = time.perf_counter()
        self.rec_btn.configure(state="normal", text="⏹   PARAR GRAVAÇÃO", fg_color="#2a3042",
                               hover_color="#363d55")
        self.set_status("●  GRAVANDO", ACCENT)
        self.info.configure(text=f"Gravando: {self.rec_target}")
        if self.cfg["minimize"]:
            self.iconify()

    def stop_recording(self):
        self.recording = False
        self.busy = True
        self.rec_btn.configure(state="disabled", text="Salvando vídeo...")
        self.set_status("●  PROCESSANDO", "#f5a623")
        video, recs, session = self.video, self.audio_recs, self.session

        def worker():
            com_init()
            video.stop()
            for _, r, _ in recs:
                r.stop()
            audios = []
            v0 = video.start_time
            for _, r, vol in recs:
                if r.start_time is None or v0 is None:
                    continue
                wav = Path(r.path)
                if wav.exists() and wav.stat().st_size > 1000:
                    audios.append((wav, v0 - r.start_time, vol))
            ok, msg = mux(session["video"], audios, session["final"])
            if ok:
                for f in session["tmp"].iterdir():
                    try:
                        f.unlink()
                    except OSError:
                        pass
                try:
                    session["tmp"].rmdir()
                except OSError:
                    pass
            self.after(0, lambda: self._stopped(ok, msg, session))

        threading.Thread(target=worker, daemon=True).start()

    def _stopped(self, ok, msg, session):
        self.busy = False
        self.video = None
        self.audio_recs = []
        self.rec_btn.configure(state="normal", text="⏺   INICIAR GRAVAÇÃO", fg_color=ACCENT,
                               hover_color=ACCENT_HOVER)
        self.meter_sys.set(0)
        self.meter_mic.set(0)
        if ok:
            self.set_status("●  SALVO", GREEN)
            self.info.configure(text=f"Salvo: {session['final'].name}")
        else:
            self.set_status("●  ERRO AO SALVAR", ACCENT)
            self.info.configure(text=f"Arquivos brutos em {session['tmp']}")
            print(msg)
        self.refresh_files()
        if self.state() == "iconic":
            self.deiconify()

    def _tick(self):
        if self.recording:
            el = time.perf_counter() - self.rec_start
            self.timer.configure(text=fmt_time(el))
            blink = int(el * 2) % 2 == 0
            self.status_dot.configure(text_color=ACCENT if blink else "#7a1d2d")
            levels = {k: r.level for k, r, _ in self.audio_recs}
            self.meter_sys.set(levels.get("sys", 0))
            self.meter_mic.set(levels.get("mic", 0))
            for _, r, _ in self.audio_recs:
                r.level *= 0.85
            if self.video and not self.video.alive():
                self.stop_recording()
            elif self.video:
                self.info.configure(text=f"{self.rec_target}  •  {self.video.fps_now:.0f} fps  •  F9 para parar")
        self.after(100, self._tick)

    def on_close(self):
        if self.recording:
            self.stop_recording()
            self.after(500, self._wait_close)
            return
        self._wait_close()

    def _wait_close(self):
        if self.busy:
            self.after(300, self._wait_close)
            return
        self.collect_cfg()
        try:
            self.pa.terminate()
        except Exception:
            pass
        self.destroy()


if __name__ == "__main__":
    try:
        # icone proprio na barra de tarefas mesmo rodando via pythonw.exe
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("GameRec.App")
    except Exception:
        pass
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except Exception:
        pass
    App().mainloop()
