namespace WinRec.Core.Media;

public static class FfmpegErrors
{
    /// <summary>Mensagens do FFmpeg que só dizem "falhou", sem a causa.</summary>
    private static readonly string[] Generic =
    [
        "conversion failed", "error opening output file", "error opening input file", "error opening output files",
        "error opening input files", "nothing was written into output file", "terminating thread with return code",
        "exiting normally",
    ];

    /// <summary>Linha que explica o erro: a primeira que não é genérica, sem o "[filtro @ 0x...]".</summary>
    public static string Summarize(string stderr)
    {
        var lines = stderr.Split('\n').Select(l => l.Trim()).Where(l => l.Length > 0).ToList();
        var line = lines.FirstOrDefault(l => !Generic.Any(g => l.Contains(g, StringComparison.OrdinalIgnoreCase)))
                   ?? lines.LastOrDefault();
        if (line is null) return "";
        var end = line.IndexOf("] ", StringComparison.Ordinal);
        if (!line.StartsWith('[') || end < 0) return line;
        var name = line[1..end].Split(" @ ")[0];
        return $"{name}: {line[(end + 2)..]}";
    }
}
