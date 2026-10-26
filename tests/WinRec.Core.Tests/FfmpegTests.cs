using WinRec.Core.Media;

namespace WinRec.Core.Tests;

public class FfmpegTests
{
    [Fact]
    public void Summarize_SkipsGenericLinesAndFilterPrefix()
    {
        const string err = "[gfxcapture @ 0000020a] Failed to create capture item\nError opening input files: I/O error\n";
        Assert.Equal("gfxcapture: Failed to create capture item", FfmpegErrors.Summarize(err));
    }

    [Fact]
    public void Summarize_OnlyGeneric_ReturnsLast()
    {
        Assert.Equal("Conversion failed!", FfmpegErrors.Summarize("Error opening output files\nConversion failed!\n"));
    }

    [Fact]
    public void Summarize_Empty() => Assert.Equal("", FfmpegErrors.Summarize("\n \n"));

    [Fact]
    public void ParseCameras_ReadsVideoDevicesOnly()
    {
        const string err = """
            [in#0 @ 000001] "Integrated Camera" (video)
            [in#0 @ 000001]   Alternative name "@device_pnp_\\?\usb#vid"
            [in#0 @ 000001] "Microfone (Realtek)" (audio)
            [dshow @ 000002] "OBS Virtual Camera" (video)
            """;
        Assert.Equal(["Integrated Camera", "OBS Virtual Camera"], Ffmpeg.ParseCameras(err));
    }

    [Fact]
    public void ParseDuration_ReadsInputDuration()
    {
        Assert.Equal(new TimeSpan(0, 1, 2, 3, 450), Ffmpeg.ParseDuration("  Duration: 01:02:03.45, start: 0"));
        Assert.Null(Ffmpeg.ParseDuration("nada"));
    }

    [Fact]
    public void Capture_Window_UsesGfxcaptureAndScale()
    {
        var args = CaptureArgs.Build(new CaptureRequest(CaptureSource.GfxWindow, "h264_nvenc", 60, 1080, 20, "out.mkv",
            Window: 1234));
        Assert.Contains("gfxcapture=hwnd=1234:max_framerate=60:width=-2:height=-2:resize_mode=scale_aspect:capture_cursor=1", args);
        Assert.Contains(args, a => a.Contains("min(ih,1080)"));
        Assert.Equal("h264_nvenc", args[args.IndexOf("-c:v") + 1]);
        Assert.Equal("out.mkv", args[^1]);
    }

    [Fact]
    public void Capture_Desktop_UsesMonitorGeometry()
    {
        var mon = new MonitorInfo(-1920, 0, 1920, 1080, false, 7);
        var args = CaptureArgs.Build(new CaptureRequest(CaptureSource.GdiDesktop, "libx264", 30, null, 24, "o.mkv",
            Monitor: mon));
        Assert.Equal("-1920", args[args.IndexOf("-offset_x") + 1]);
        Assert.Equal("1920x1080", args[args.IndexOf("-video_size") + 1]);
        Assert.Equal("24", args[args.IndexOf("-crf") + 1]);
    }

    [Fact]
    public void Capture_Camera_UsesDshowDeviceName()
    {
        var args = CaptureArgs.Build(new CaptureRequest(CaptureSource.Camera, "libx264", 30, null, 20, "o.mkv",
            Camera: "HD Webcam"));
        Assert.Equal("dshow", args[args.IndexOf("-f") + 1]);
        Assert.Contains("video=HD Webcam", args);
    }

    [Fact]
    public void Mux_NoAudio_CopiesVideoOnly()
    {
        var args = MuxArgs.Build("v.mkv", [], "o.mp4");
        Assert.DoesNotContain("-filter_complex", args);
        Assert.Equal(["-map", "0:v"], args[args.IndexOf("-map")..(args.IndexOf("-map") + 2)]);
    }

    [Fact]
    public void Mux_TwoTracks_AlignsAndMixes()
    {
        var args = MuxArgs.Build("v.mkv", [new AudioTrack("s.wav", 0.25, 100), new AudioTrack("m.wav", -0.5, 150)], "o.mp4");
        var filter = args[args.IndexOf("-filter_complex") + 1];
        Assert.Equal(
            "[1:a]atrim=start=0.2500,asetpts=PTS-STARTPTS,aresample=48000,aformat=channel_layouts=stereo,volume=1.00[a1];"
            + "[2:a]adelay=500:all=1,aresample=48000,aformat=channel_layouts=stereo,volume=1.50[a2];"
            + "[a1][a2]amix=inputs=2:duration=longest:normalize=0[aout]",
            filter);
        Assert.Equal("[aout]", args[args.LastIndexOf("-map") + 1]);
    }
}
