using System.Globalization;

namespace WinRec.Core.Media;

public enum CaptureSource
{
    /// <summary>Windows Graphics Capture de uma janela (sem o que estiver por cima).</summary>
    GfxWindow,
    GfxMonitor,
    Ddagrab,
    GdiWindow,
    GdiDesktop,
    /// <summary>Webcam via DirectShow.</summary>
    Camera,
}

public sealed record MonitorInfo(int X, int Y, int Width, int Height, bool Primary, nint Handle);

public sealed record CaptureRequest(
    CaptureSource Source,
    string Encoder,
    int Fps,
    int? MaxHeight,
    int Quality,
    string Output,
    int MonitorIndex = 0,
    MonitorInfo? Monitor = null,
    nint Window = 0,
    string Camera = "");

public static class CaptureArgs
{
    public static string Label(CaptureSource s) => s switch
    {
        CaptureSource.GfxWindow => "captura de janela (WGC)",
        CaptureSource.GfxMonitor => "captura de tela (WGC)",
        CaptureSource.Ddagrab => "captura de tela (DXGI)",
        CaptureSource.GdiWindow => "captura de janela (GDI)",
        CaptureSource.GdiDesktop => "captura de tela (GDI)",
        _ => "webcam (DirectShow)",
    };

    /// <summary>Linha de comando do FFmpeg que grava só o vídeo (o áudio é juntado depois, em <see cref="MuxArgs"/>).</summary>
    public static List<string> Build(CaptureRequest c)
    {
        var fps = c.Fps.ToString(CultureInfo.InvariantCulture);
        List<string> a = ["-loglevel", "error", "-nostats", "-y", "-progress", "pipe:1", "-stats_period", "0.1"];
        var scale = c.MaxHeight is { } h
            ? $"scale=-2:'min(ih,{h})':flags=bicubic:out_color_matrix=bt709:out_range=tv"
            : "scale=out_color_matrix=bt709:out_range=tv";
        string vf;
        switch (c.Source)
        {
            case CaptureSource.GfxWindow or CaptureSource.GfxMonitor:
                var target = c.Source == CaptureSource.GfxWindow ? $"hwnd={c.Window}" : $"hmonitor={c.Monitor!.Handle}";
                a.AddRange(["-f", "lavfi", "-i",
                    $"gfxcapture={target}:max_framerate={fps}:width=-2:height=-2:resize_mode=scale_aspect:capture_cursor=1"]);
                vf = $"hwdownload,format=bgra,{scale},format=nv12";
                break;
            case CaptureSource.Ddagrab:
                a.AddRange(["-f", "lavfi", "-i", $"ddagrab=output_idx={c.MonitorIndex}:framerate={fps}:draw_mouse=1"]);
                vf = $"hwdownload,format=bgra,{scale},format=nv12";
                break;
            case CaptureSource.GdiWindow:
                a.AddRange(["-f", "gdigrab", "-framerate", fps, "-draw_mouse", "1", "-i", $"hwnd={c.Window}"]);
                vf = $"crop=trunc(iw/2)*2:trunc(ih/2)*2,{scale},format=nv12";
                break;
            case CaptureSource.GdiDesktop:
                var m = c.Monitor!;
                a.AddRange(["-f", "gdigrab", "-framerate", fps, "-draw_mouse", "1",
                    "-offset_x", m.X.ToString(), "-offset_y", m.Y.ToString(),
                    "-video_size", $"{m.Width}x{m.Height}", "-i", "desktop"]);
                vf = $"{scale},format=nv12";
                break;
            default:
                // buffer grande: webcams USB entregam rajadas e o DirectShow descarta quadros com buffer cheio
                a.AddRange(["-f", "dshow", "-rtbufsize", "256M", "-i", $"video={c.Camera}"]);
                vf = $"crop=trunc(iw/2)*2:trunc(ih/2)*2,{scale},format=nv12";
                break;
        }
        a.AddRange(["-vf", vf, "-fps_mode", "cfr", "-r", fps]);
        a.AddRange(Encoders.Args(c.Encoder, c.Quality, c.Fps));
        a.AddRange(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"]);
        a.Add(c.Output);
        return a;
    }

    /// <summary>Prévia da webcam: quadros BGRA de tamanho fixo no stdout.</summary>
    public static List<string> CameraPreview(string camera, int width, int height) =>
    [
        "-loglevel", "error", "-f", "dshow", "-rtbufsize", "64M", "-i", $"video={camera}",
        "-vf", $"scale={width}:{height}:force_original_aspect_ratio=decrease,"
               + $"pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,format=bgra",
        "-r", "30", "-f", "rawvideo", "pipe:1",
    ];
}
