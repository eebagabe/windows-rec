using System.Runtime.InteropServices;
using WinRec.Core;

namespace WinRec.Core.Tests;

public class AudioLevelTests
{
    [Fact]
    public void Report_KeepsMaxUntilTake()
    {
        var level = new AudioLevel();
        level.Report(0.3f);
        level.Report(0.8f);
        level.Report(0.5f);
        Assert.Equal(0.8f, level.Take());
        Assert.Equal(0f, level.Take());
    }

    [Fact]
    public void Report_ConcurrentWriters_KeepMax()
    {
        var level = new AudioLevel();
        Parallel.For(0, 1000, i => level.Report(i / 1000f));
        Assert.Equal(0.999f, level.Take());
    }

    [Fact]
    public void Peak_Float32()
    {
        float[] samples = [0.1f, -0.7f, 0.4f];
        Assert.Equal(0.7f, AudioLevel.Peak(MemoryMarshal.AsBytes(samples.AsSpan()), true, 32));
    }

    [Fact]
    public void Peak_Pcm16()
    {
        short[] samples = [100, -16384, 200];
        Assert.Equal(0.5f, AudioLevel.Peak(MemoryMarshal.AsBytes(samples.AsSpan()), false, 16));
    }
}
