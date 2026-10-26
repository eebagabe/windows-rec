using System.ComponentModel;
using System.Diagnostics;
using System.Text.RegularExpressions;

namespace WinRec.Core.Media;

/// <summary>Localiza e executa o ffmpeg.exe distribuído junto com o app (bin\ffmpeg.exe).</summary>
public static partial class Ffmpeg
{
    public static string ExePath { get; } = Find();

    private static string Find()
    {
        var dir = AppContext.BaseDirectory;
        string[] candidates = [Path.Combine(dir, "bin", "ffmpeg.exe"), Path.Combine(dir, "ffmpeg.exe")];
        return candidates.FirstOrDefault(File.Exists) ?? "ffmpeg.exe";
    }

    public static ProcessStartInfo StartInfo(IEnumerable<string> args)
    {
        var psi = new ProcessStartInfo(ExePath)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        psi.ArgumentList.Add("-hide_banner");
        foreach (var a in args) psi.ArgumentList.Add(a);
        return psi;
    }

    /// <summary>Roda até o fim e devolve (código de saída, stdout em bytes, stderr).</summary>
    public static async Task<(int Code, byte[] Stdout, string Stderr)> RunAsync(
        IEnumerable<string> args, TimeSpan timeout)
    {
        using var p = Process.Start(StartInfo(args))!;
        p.StandardInput.Close();
        using var cts = new CancellationTokenSource(timeout);
        var stdout = new MemoryStream();
        var outTask = p.StandardOutput.BaseStream.CopyToAsync(stdout, cts.Token);
        var errTask = p.StandardError.ReadToEndAsync(cts.Token);
        try
        {
            await Task.WhenAll(outTask, errTask, p.WaitForExitAsync(cts.Token));
        }
        catch (OperationCanceledException)
        {
            p.Kill(entireProcessTree: true);
            return (-1, [], "tempo esgotado");
        }
        return (p.ExitCode, stdout.ToArray(), await errTask);
    }

    /// <summary>Primeiro encoder de GPU que funciona (testados em paralelo); senão x264.</summary>
    public static async Task<string> DetectEncoderAsync()
    {
        var tests = Encoders.Gpu.Select(async enc =>
        {
            try
            {
                var (code, _, _) = await RunAsync(
                    ["-loglevel", "error", "-f", "lavfi", "-i", "color=black:s=1280x720:r=30",
                     "-frames:v", "10", "-pix_fmt", "nv12", "-c:v", enc, "-f", "null", "-"],
                    TimeSpan.FromSeconds(20));
                return code == 0;
            }
            catch (Win32Exception)
            {
                return false;
            }
        }).ToArray();
        var ok = await Task.WhenAll(tests);
        var i = Array.IndexOf(ok, true);
        return i >= 0 ? Encoders.Gpu[i] : Encoders.Cpu;
    }

    /// <summary>Webcams (DirectShow) instaladas.</summary>
    public static async Task<IReadOnlyList<string>> ListCamerasAsync()
    {
        try
        {
            var (_, _, err) = await RunAsync(["-list_devices", "true", "-f", "dshow", "-i", "dummy"],
                TimeSpan.FromSeconds(10));
            return ParseCameras(err);
        }
        catch (Win32Exception)
        {
            return [];
        }
    }

    internal static IReadOnlyList<string> ParseCameras(string stderr) =>
        CameraLine().Matches(stderr).Select(m => m.Groups[1].Value).Distinct().ToList();

    [GeneratedRegex("""\] "(.+)" \(video\)""")]
    private static partial Regex CameraLine();

    /// <summary>Miniatura (JPEG) e duração de um vídeo, numa única execução do FFmpeg.</summary>
    public static async Task<(byte[]? Jpeg, TimeSpan? Duration)> ProbeAsync(string video, int width)
    {
        try
        {
            var (code, jpeg, err) = await RunAsync(
                ["-ss", "1", "-i", video, "-frames:v", "1", "-vf", $"scale={width}:-2",
                 "-f", "image2pipe", "-c:v", "mjpeg", "-q:v", "4", "pipe:1"],
                TimeSpan.FromSeconds(15));
            return (code == 0 && jpeg.Length > 0 ? jpeg : null, ParseDuration(err));
        }
        catch (Win32Exception)
        {
            return (null, null);
        }
    }

    internal static TimeSpan? ParseDuration(string stderr) =>
        DurationLine().Match(stderr) is { Success: true } m && TimeSpan.TryParse(m.Groups[1].Value, out var d)
            ? d
            : null;

    [GeneratedRegex(@"Duration: (\d+:\d\d:\d\d(?:\.\d+)?)")]
    private static partial Regex DurationLine();

    /// <summary>Dica para quando o Windows não deixa o FFmpeg rodar.</summary>
    public static string SpawnHint(Win32Exception e) => e.NativeErrorCode switch
    {
        // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION / ERROR_ACCESS_DISABLED_BY_POLICY
        4551 or 1260 => "O Windows bloqueou o FFmpeg (Smart App Control ou política de segurança). Desligue o Smart "
                        + "App Control em Segurança do Windows > Controle de aplicativos e navegador e reinstale o WinRec.",
        2 or 3 => "O arquivo não existe. O antivírus pode ter apagado o FFmpeg: reinstale o WinRec.",
        5 or 225 => "Acesso negado: o antivírus bloqueou o FFmpeg. Libere o arquivo no antivírus e reinstale o WinRec.",
        _ => "O antivírus pode ter apagado ou bloqueado o FFmpeg. Reinstale o WinRec ou libere o arquivo no antivírus.",
    };
}
