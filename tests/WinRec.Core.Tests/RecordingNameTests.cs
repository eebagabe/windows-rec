using WinRec.Core;

namespace WinRec.Core.Tests;

public class RecordingNameTests
{
    [Fact]
    public void FileName_RoundTripsDate()
    {
        var when = new DateTime(2026, 10, 15, 14, 3, 22);
        var name = RecordingName.FileName(RecordingName.Stamp(when));
        Assert.Equal("Gravacao_2026-10-15_14-03-22.mp4", name);
        Assert.Equal(when, RecordingName.ParseDate(name));
    }

    [Theory]
    [InlineData("video.mp4")]
    [InlineData("Gravacao_ontem.mp4")]
    public void ParseDate_OtherNames_ReturnsNull(string name) => Assert.Null(RecordingName.ParseDate(name));
}
