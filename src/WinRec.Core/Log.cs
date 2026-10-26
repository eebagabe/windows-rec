namespace WinRec.Core;

/// <summary>Log de diagnóstico em %APPDATA%\WinRec\winrec.log (para investigar falhas em outras máquinas).</summary>
public static class Log
{
    private const long MaxSize = 1024 * 1024;
    private static readonly Lock Gate = new();
    private static StreamWriter? _writer;

    public static void Write(string message)
    {
        lock (Gate)
        {
            try
            {
                _writer ??= Open();
                _writer.WriteLine($"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] {message}");
            }
            catch (IOException)
            {
                // log nunca derruba o app
            }
        }
    }

    private static StreamWriter Open()
    {
        var path = AppPaths.LogFile;
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        // mantém o anterior como .old.log quando passa do limite
        if (File.Exists(path) && new FileInfo(path).Length > MaxSize)
            File.Move(path, Path.ChangeExtension(path, ".old.log"), overwrite: true);
        return new StreamWriter(path, append: true) { AutoFlush = true };
    }
}
