namespace WinRec.Core;

/// <summary>Pastas do app: configuração e log em %APPDATA%\WinRec, cache em %LOCALAPPDATA%\WinRec.</summary>
public static class AppPaths
{
    public static string ConfigDir { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "WinRec");

    public static string CacheDir { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "WinRec");

    public static string ConfigFile => Path.Combine(ConfigDir, "config.json");

    public static string LogFile => Path.Combine(ConfigDir, "winrec.log");

    /// <summary>Vídeos\WinRec (respeita a pasta Vídeos redirecionada, ex.: OneDrive).</summary>
    public static string DefaultOutputDir { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyVideos), "WinRec");
}
