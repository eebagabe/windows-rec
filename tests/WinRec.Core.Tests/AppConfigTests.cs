using WinRec.Core;

namespace WinRec.Core.Tests;

public sealed class AppConfigTests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"winrec-{Guid.NewGuid():N}.json");

    public void Dispose() => File.Delete(_path);

    [Fact]
    public void Load_MissingFile_ReturnsDefaults()
    {
        Assert.Equal(new AppConfig(), AppConfig.Load(_path));
    }

    [Fact]
    public void SaveThenLoad_RoundTrips()
    {
        var cfg = new AppConfig { Fps = 30, Quality = "Leve", Mic = true, MicDevice = "USB", Source = "win:obs.exe" };
        cfg.Save(_path);
        Assert.Equal(cfg, AppConfig.Load(_path));
    }

    [Fact]
    public void Load_InvalidValues_FallBackToDefaults()
    {
        File.WriteAllText(_path, """{"Fps":144,"Quality":"x","Resolution":"8k","OutputDir":"","SystemVolume":999,"MicVolume":-5}""");
        var cfg = AppConfig.Load(_path);
        var def = new AppConfig();
        Assert.Equal(def.Fps, cfg.Fps);
        Assert.Equal(def.Quality, cfg.Quality);
        Assert.Equal(def.Resolution, cfg.Resolution);
        Assert.Equal(def.OutputDir, cfg.OutputDir);
        Assert.Equal(200, cfg.SystemVolume);
        Assert.Equal(0, cfg.MicVolume);
    }

    [Fact]
    public void Load_CorruptJson_ReturnsDefaults()
    {
        File.WriteAllText(_path, "{ not json");
        Assert.Equal(new AppConfig(), AppConfig.Load(_path));
    }

    [Theory]
    [InlineData("Máxima", 16)]
    [InlineData("Leve", 28)]
    public void Crf_MapsQuality(string quality, int crf) => Assert.Equal(crf, new AppConfig { Quality = quality }.Crf);

    [Fact]
    public void MaxHeight_NativeIsNull()
    {
        Assert.Null(new AppConfig { Resolution = "Nativa" }.MaxHeight);
        Assert.Equal(720, new AppConfig { Resolution = "720p" }.MaxHeight);
    }

    [Theory]
    [InlineData("auto", "auto", "")]
    [InlineData("mon:1", "mon", "1")]
    [InlineData("cam:HD Webcam: C920", "cam", "HD Webcam: C920")]
    public void SourceKey_Parse(string key, string kind, string value) =>
        Assert.Equal((kind, value), SourceKey.Parse(key));
}
