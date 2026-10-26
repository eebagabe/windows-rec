using System.Diagnostics;

namespace WinRec;

/// <summary>Abre arquivos, pastas e links com o programa padrão do Windows.</summary>
internal static class Shell
{
    public static void Open(string target)
    {
        try
        {
            Process.Start(new ProcessStartInfo(target) { UseShellExecute = true })?.Dispose();
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or FileNotFoundException)
        {
            Core.Log.Write($"não abriu {target}: {e.Message}");
        }
    }

    /// <summary>Abre o Explorer com o arquivo selecionado.</summary>
    public static void ShowInFolder(string file) =>
        Process.Start("explorer.exe", $"/select,\"{file}\"")?.Dispose();
}
