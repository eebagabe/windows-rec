using System.Globalization;

namespace WinRec.Core;

/// <summary>Nome dos arquivos gravados: "Gravacao_AAAA-MM-DD_HH-MM-SS.mp4".</summary>
public static class RecordingName
{
    private const string Prefix = "Gravacao_";
    private const string StampFormat = "yyyy-MM-dd_HH-mm-ss";

    public static string Stamp(DateTime when) => when.ToString(StampFormat, CultureInfo.InvariantCulture);

    public static string FileName(string stamp) => $"{Prefix}{stamp}.mp4";

    /// <summary>Data da gravação a partir do nome, se seguir o padrão.</summary>
    public static DateTime? ParseDate(string fileName)
    {
        var stem = Path.GetFileNameWithoutExtension(fileName);
        return stem.StartsWith(Prefix, StringComparison.Ordinal)
               && DateTime.TryParseExact(stem[Prefix.Length..], StampFormat, CultureInfo.InvariantCulture,
                   DateTimeStyles.None, out var when)
            ? when
            : null;
    }
}
