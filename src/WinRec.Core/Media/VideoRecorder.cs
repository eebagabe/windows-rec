using System.Diagnostics;
using System.Globalization;

namespace WinRec.Core.Media;

/// <summary>Processo do FFmpeg gravando vídeo; acompanha o progresso pelo "-progress pipe:1".</summary>
public sealed class VideoRecorder : IDisposable
{
    private readonly Process _process;
    private readonly Thread _progressReader;
    private readonly Task<string> _stderr;
    private long _frames;
    private float _fps;
    private double _start = double.NaN;

    private VideoRecorder(Process process)
    {
        _process = process;
        _stderr = process.StandardError.ReadToEndAsync();
        _progressReader = new Thread(ReadProgress) { IsBackground = true, Name = "ffmpeg-progress" };
        _progressReader.Start();
    }

    /// <exception cref="System.ComponentModel.Win32Exception">O Windows não deixou o FFmpeg rodar.</exception>
    public static VideoRecorder Start(IReadOnlyList<string> args)
    {
        Log.Write($"ffmpeg {string.Join(' ', args)}");
        return new VideoRecorder(Process.Start(Ffmpeg.StartInfo(args))!);
    }

    public long Frames => Interlocked.Read(ref _frames);

    public float Fps => Volatile.Read(ref _fps);

    /// <summary>Estimativa (relógio do app) do instante do primeiro quadro.</summary>
    public double? StartTime => double.IsNaN(Volatile.Read(ref _start)) ? null : _start;

    public bool Alive => !_process.HasExited;

    /// <summary>Saída de erro completa (válida depois de <see cref="Stop"/> ou <see cref="Kill"/>).</summary>
    public string Stderr => _stderr.IsCompleted ? _stderr.Result : "";

    /// <summary>Encerra na hora (tentativa de captura que não iniciou).</summary>
    public void Kill()
    {
        if (Alive) _process.Kill(entireProcessTree: true);
        Finish(TimeSpan.FromSeconds(5));
    }

    /// <summary>Pede para o FFmpeg finalizar o arquivo ("q") e espera.</summary>
    public void Stop()
    {
        if (Alive)
        {
            try
            {
                _process.StandardInput.Write('q');
                _process.StandardInput.Flush();
            }
            catch (IOException)
            {
                // já fechou
            }
            if (!_process.WaitForExit(TimeSpan.FromSeconds(15))) _process.Kill(entireProcessTree: true);
        }
        Finish(TimeSpan.FromSeconds(5));
    }

    private void Finish(TimeSpan timeout)
    {
        _process.WaitForExit(timeout);
        _progressReader.Join(timeout);
        _stderr.Wait(timeout);
    }

    public void Dispose()
    {
        if (Alive) _process.Kill(entireProcessTree: true);
        _process.Dispose();
    }

    private void ReadProgress()
    {
        long frame = 0, outUs = 0;
        float fps = 0;
        var inv = CultureInfo.InvariantCulture;
        while (_process.StandardOutput.ReadLine() is { } line)
        {
            var eq = line.IndexOf('=');
            if (eq < 0) continue;
            var (k, v) = (line[..eq].Trim(), line[(eq + 1)..].Trim());
            switch (k)
            {
                case "frame": long.TryParse(v, inv, out frame); break;
                case "out_time_us": long.TryParse(v, inv, out outUs); break;
                case "fps": float.TryParse(v, inv, out fps); break;
                case "progress":
                    if (frame > 0 && outUs > 0)
                    {
                        // o menor valor observado é o mais próximo do real (menos atraso de buffer)
                        var est = AppClock.Now - outUs / 1e6;
                        if (double.IsNaN(_start) || est < _start) Volatile.Write(ref _start, est);
                    }
                    Interlocked.Exchange(ref _frames, frame);
                    Volatile.Write(ref _fps, fps);
                    break;
            }
        }
    }
}
