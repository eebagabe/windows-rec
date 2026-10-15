//! Win32: monitores, janelas (deteccao de jogo), hotkey global e utilitarios do shell.
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::mpsc;

use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT, SYSTEMTIME};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_NOREPEAT};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetMessageW, GetWindow, GetWindowLongW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, MessageBoxW, SetForegroundWindow, ShowWindow,
    GWL_EXSTYLE, GW_OWNER, MB_ICONERROR, MSG, SW_MINIMIZE, SW_RESTORE, SW_SHOWNORMAL, WM_HOTKEY, WS_EX_TOOLWINDOW,
};

#[derive(Clone, Debug)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
    pub hmon: isize,
}

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
    pub score: i32,
    pub iconic: bool,
}

/// Processos que nunca sao jogos.
const NOT_GAMES: &[&str] = &[
    "explorer.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "windowsterminal.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "textinputhost.exe",
    "python.exe",
    "pythonw.exe",
    "gamerec.exe",
    "shellexperiencehost.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "lockapp.exe",
    "epicgameslauncher.exe",
    "riotclientux.exe",
    "riotclientservices.exe",
    "battle.net.exe",
    "galaxyclient.exe",
    "eadesktop.exe",
    "ubisoftconnect.exe",
    "upc.exe",
    "discord.exe",
    "obs64.exe",
    "taskmgr.exe",
    "code.exe",
    "nvidia app.exe",
    "nvcontainer.exe",
    "cmd.exe",
    "powershell.exe",
    "conhost.exe",
    "notepad.exe",
    "mmc.exe",
];
const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "opera.exe",
    "brave.exe",
    "spotify.exe",
    "whatsapp.exe",
    "teams.exe",
    "ms-teams.exe",
    "slack.exe",
    "telegram.exe",
];
const GAME_PATHS: &[&str] = &[
    "steamapps",
    "epic games",
    "riot games",
    "gog galaxy",
    "xboxgames",
    "\\games\\",
    "ubisoft game launcher\\games",
    "ea games",
    "battle.net",
    "rockstar games",
    "minecraft",
];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

// ------------------------------------------------------------------ monitores
unsafe extern "system" fn monitor_cb(hmon: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    let mons = &mut *(lp as *mut Vec<Monitor>);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(hmon, &mut info) != 0 {
        let r = info.rcMonitor;
        mons.push(Monitor {
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
            primary: info.dwFlags & 1 != 0,
            hmon: hmon as isize,
        });
    }
    1
}

/// Monitores, com o principal primeiro.
pub fn list_monitors() -> Vec<Monitor> {
    let mut mons: Vec<Monitor> = Vec::new();
    unsafe {
        EnumDisplayMonitors(null_mut(), null(), Some(monitor_cb), &mut mons as *mut _ as LPARAM);
    }
    mons.sort_by_key(|m| !m.primary);
    if mons.is_empty() {
        mons.push(Monitor { x: 0, y: 0, w: 1920, h: 1080, primary: true, hmon: 0 });
    }
    mons
}

// ------------------------------------------------------------------ janelas
fn exe_path(pid: u32) -> String {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..size as usize])
    }
}

struct EnumCtx<'a> {
    monitors: &'a [Monitor],
    own_pid: u32,
    out: Vec<WindowInfo>,
}

unsafe extern "system" fn window_cb(hwnd: HWND, lp: LPARAM) -> BOOL {
    let ctx = &mut *(lp as *mut EnumCtx);
    if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
        return 1;
    }
    if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    let mut cloaked: i32 = 0;
    DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as _, &mut cloaked as *mut i32 as *mut c_void, 4);
    if cloaked != 0 {
        return 1;
    }
    let n = GetWindowTextLengthW(hwnd);
    if n <= 0 {
        return 1;
    }
    let mut title = vec![0u16; n as usize + 1];
    let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    let title = String::from_utf16_lossy(&title[..len.max(0) as usize]);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == ctx.own_pid {
        return 1;
    }
    let path = exe_path(pid);
    let low = path.to_lowercase();
    let exe = low.rsplit('\\').next().unwrap_or("").to_string();
    if exe.is_empty() || NOT_GAMES.contains(&exe.as_str()) {
        return 1;
    }
    let mut r: RECT = zeroed();
    GetWindowRect(hwnd, &mut r);
    let iconic = IsIconic(hwnd) != 0;
    let mut score = 0;
    if GAME_PATHS.iter().any(|p| low.contains(p)) {
        score += 3;
    }
    if BROWSERS.contains(&exe.as_str()) {
        score -= 5;
    }
    let fullscreen =
        ctx.monitors.iter().any(|m| r.left <= m.x && r.top <= m.y && r.right >= m.x + m.w && r.bottom >= m.y + m.h);
    if !iconic && fullscreen {
        score += 2;
    }
    ctx.out.push(WindowInfo { hwnd: hwnd as isize, title, exe, score, iconic });
    1
}

/// Janelas visiveis de aplicativos, ordenadas pela pontuacao de "parece jogo".
pub fn list_windows(monitors: &[Monitor]) -> Vec<WindowInfo> {
    let mut ctx = EnumCtx { monitors, own_pid: std::process::id(), out: Vec::new() };
    unsafe {
        EnumWindows(Some(window_cb), &mut ctx as *mut _ as LPARAM);
    }
    ctx.out.sort_by_key(|w| -w.score);
    ctx.out
}

/// Janela em foco (se parece aplicativo) > maior pontuacao de jogo.
pub fn best_game(windows: &[WindowInfo], fg: Option<isize>) -> Option<&WindowInfo> {
    if let Some(fg) = fg {
        if let Some(w) = windows.iter().find(|w| w.hwnd == fg && w.score >= 0) {
            return Some(w);
        }
    }
    windows.iter().find(|w| w.score >= 2)
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

pub fn minimize(hwnd: isize) {
    if hwnd != 0 {
        unsafe { ShowWindow(hwnd as HWND, SW_MINIMIZE) };
    }
}

pub fn restore(hwnd: isize) {
    if hwnd != 0 {
        unsafe {
            if IsIconic(hwnd as HWND) != 0 {
                ShowWindow(hwnd as HWND, SW_RESTORE);
                SetForegroundWindow(hwnd as HWND);
            }
        }
    }
}

pub fn message_box(title: &str, text: &str) {
    let (t, m) = (wide(title), wide(text));
    unsafe { MessageBoxW(null_mut(), m.as_ptr(), t.as_ptr(), MB_ICONERROR) };
}

// ------------------------------------------------------------------ hotkey
/// Registra uma hotkey global numa thread propria; retorna se o registro funcionou.
pub fn spawn_hotkey(vk: u32, on_press: impl Fn() + Send + 'static) -> bool {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("hotkey".into())
        .spawn(move || unsafe {
            let ok = RegisterHotKey(null_mut(), 1, MOD_NOREPEAT, vk) != 0;
            let _ = tx.send(ok);
            if !ok {
                return;
            }
            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY {
                    on_press();
                }
            }
        })
        .ok()
        .and_then(|_| rx.recv().ok())
        .unwrap_or(false)
}

// ------------------------------------------------------------------ shell
/// Abre arquivo, pasta ou URL com o programa padrao.
pub fn open(target: &str) {
    let op = wide("open");
    let file = wide(target);
    unsafe {
        ShellExecuteW(null_mut(), op.as_ptr(), file.as_ptr(), null(), null(), SW_SHOWNORMAL);
    }
}

fn now_local() -> SYSTEMTIME {
    let mut t: SYSTEMTIME = unsafe { zeroed() };
    unsafe { GetLocalTime(&mut t) };
    t
}

/// "AAAA-MM-DD_HH-MM-SS" no horario local.
pub fn local_stamp() -> String {
    let t = now_local();
    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// "AAAA-MM-DD HH:MM:SS.mmm" no horario local (log).
pub fn local_time() -> String {
    let t = now_local();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}
