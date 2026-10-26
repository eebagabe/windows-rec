using System.Text.Json;
using System.Text.Json.Serialization;

namespace WinRec.Core;

/// <summary>Configuração persistida em %APPDATA%\WinRec\config.json.</summary>
public sealed record AppConfig
{
    public static readonly IReadOnlyList<(string Name, int Quality)> Qualities =
        [("Máxima", 16), ("Alta", 20), ("Média", 24), ("Leve", 28)];

    public static readonly IReadOnlyList<(string Name, int? MaxHeight)> Resolutions =
        [("Nativa", null), ("1440p", 1440), ("1080p", 1080), ("720p", 720)];

    public static readonly IReadOnlyList<int> FrameRates = [30, 60];

    public string OutputDir { get; set; } = AppPaths.DefaultOutputDir;

    /// <summary>Ver <see cref="SourceKey"/>.</summary>
    public string Source { get; set; } = SourceKey.Monitor(0);

    public int Fps { get; set; } = 60;
    public string Quality { get; set; } = "Alta";
    public string Resolution { get; set; } = "Nativa";
    public bool SystemAudio { get; set; } = true;
    public double SystemVolume { get; set; } = 100;
    public bool Mic { get; set; }
    public string MicDevice { get; set; } = "";
    public double MicVolume { get; set; } = 100;
    public bool MinimizeOnRecord { get; set; }

    [JsonIgnore]
    public int Crf => Qualities.FirstOrDefault(q => q.Name == Quality) is { Name: not null } q ? q.Quality : 20;

    [JsonIgnore]
    public int? MaxHeight => Resolutions.FirstOrDefault(r => r.Name == Resolution).MaxHeight;

    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    /// <summary>Lê o arquivo; valores ausentes ou inválidos voltam ao padrão.</summary>
    public static AppConfig Load(string path)
    {
        AppConfig cfg;
        try
        {
            cfg = JsonSerializer.Deserialize<AppConfig>(File.ReadAllText(path)) ?? new();
        }
        catch (Exception e) when (e is IOException or JsonException or UnauthorizedAccessException)
        {
            cfg = new();
        }
        var def = new AppConfig();
        if (!FrameRates.Contains(cfg.Fps)) cfg.Fps = def.Fps;
        if (Qualities.All(q => q.Name != cfg.Quality)) cfg.Quality = def.Quality;
        if (Resolutions.All(r => r.Name != cfg.Resolution)) cfg.Resolution = def.Resolution;
        if (string.IsNullOrWhiteSpace(cfg.OutputDir)) cfg.OutputDir = def.OutputDir;
        if (string.IsNullOrWhiteSpace(cfg.Source)) cfg.Source = def.Source;
        cfg.MicDevice ??= "";
        cfg.SystemVolume = Math.Clamp(cfg.SystemVolume, 0, 200);
        cfg.MicVolume = Math.Clamp(cfg.MicVolume, 0, 200);
        return cfg;
    }

    public void Save(string path)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            File.WriteAllText(path, JsonSerializer.Serialize(this, Json));
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            Log.Write($"config não salva: {e.Message}");
        }
    }
}

/// <summary>Chaves de fonte de gravação: "auto", "mon:&lt;índice&gt;", "win:&lt;exe&gt;" ou "cam:&lt;nome&gt;".</summary>
public static class SourceKey
{
    public const string Auto = "auto";
    public static string Monitor(int index) => $"mon:{index}";
    public static string Window(string exe) => $"win:{exe}";
    public static string Camera(string name) => $"cam:{name}";

    public static (string Kind, string Value) Parse(string key)
    {
        var i = key.IndexOf(':');
        return i < 0 ? (key, "") : (key[..i], key[(i + 1)..]);
    }
}
