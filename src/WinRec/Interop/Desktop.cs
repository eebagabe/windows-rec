using System.Text;
using WinRec.Core.Media;
using static WinRec.Interop.Native;

namespace WinRec.Interop;

public sealed record WindowInfo(nint Handle, string Title, string Exe, bool Minimized);

/// <summary>Monitores e janelas que podem ser gravados.</summary>
public static class Desktop
{
    /// <summary>Processos do próprio Windows que nunca são alvo de gravação.</summary>
    private static readonly HashSet<string> Ignored = new(StringComparer.OrdinalIgnoreCase)
    {
        "explorer.exe", "textinputhost.exe", "applicationframehost.exe", "shellexperiencehost.exe",
        "searchhost.exe", "startmenuexperiencehost.exe", "lockapp.exe", "systemsettings.exe", "winrec.exe",
    };

    /// <summary>Monitores, com o principal primeiro.</summary>
    public static List<MonitorInfo> Monitors()
    {
        var list = new List<MonitorInfo>();
        EnumDisplayMonitors(0, 0, (h, _, _, _) =>
        {
            var info = new MONITORINFO { cbSize = System.Runtime.InteropServices.Marshal.SizeOf<MONITORINFO>() };
            if (GetMonitorInfo(h, ref info))
            {
                var r = info.rcMonitor;
                list.Add(new MonitorInfo(r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top,
                    (info.dwFlags & MONITORINFOF_PRIMARY) != 0, h));
            }
            return true;
        }, 0);
        if (list.Count == 0) list.Add(new MonitorInfo(0, 0, 1920, 1080, true, 0));
        return [.. list.OrderByDescending(m => m.Primary)];
    }

    /// <summary>Janelas de aplicativos visíveis na barra de tarefas (sem as do próprio app).</summary>
    public static List<WindowInfo> Windows()
    {
        var own = (uint)Environment.ProcessId;
        var list = new List<WindowInfo>();
        EnumWindows((h, _) =>
        {
            if (Describe(h, own) is { } w) list.Add(w);
            return true;
        }, 0);
        return list;
    }

    /// <summary>A janela, se for de um aplicativo gravável.</summary>
    public static WindowInfo? Describe(nint h, uint ownPid = 0)
    {
        if (h == 0 || !IsWindowVisible(h) || GetWindow(h, GW_OWNER) != 0) return null;
        if ((GetWindowLongPtr(h, GWL_EXSTYLE) & WS_EX_TOOLWINDOW) != 0) return null;
        if (DwmGetWindowAttribute(h, DWMWA_CLOAKED, out var cloaked, sizeof(int)) == 0 && cloaked != 0) return null;
        var len = GetWindowTextLength(h);
        if (len <= 0) return null;
        var title = new StringBuilder(len + 1);
        GetWindowText(h, title, title.Capacity);
        GetWindowThreadProcessId(h, out var pid);
        if (pid == (ownPid == 0 ? (uint)Environment.ProcessId : ownPid)) return null;
        var exe = Path.GetFileName(ExePath(pid)).ToLowerInvariant();
        if (exe.Length == 0 || Ignored.Contains(exe)) return null;
        return new WindowInfo(h, title.ToString(), exe, IsIconic(h));
    }

    private static string ExePath(uint pid)
    {
        var h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (h == 0) return "";
        try
        {
            var buf = new StringBuilder(1024);
            var size = buf.Capacity;
            return QueryFullProcessImageName(h, 0, buf, ref size) ? buf.ToString() : "";
        }
        finally
        {
            CloseHandle(h);
        }
    }

    public static nint ForegroundWindow() => GetForegroundWindow();

    public static void Minimize(nint h)
    {
        if (h != 0) ShowWindow(h, SW_MINIMIZE);
    }

    public static void Restore(nint h)
    {
        if (h == 0 || !IsIconic(h)) return;
        ShowWindow(h, SW_RESTORE);
        SetForegroundWindow(h);
    }
}
