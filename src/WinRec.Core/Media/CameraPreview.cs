using System.Diagnostics;

namespace WinRec.Core.Media;

/// <summary>Lê quadros BGRA da webcam pelo FFmpeg, para mostrar uma prévia.</summary>
public sealed class CameraPreview : IDisposable
{
    private readonly Process _process;
    private readonly Thread _reader;
    private readonly Task<string> _stderr;

    public int Width { get; }
    public int Height { get; }

    /// <summary>Chamado na thread de leitura com o quadro (o buffer é reutilizado).</summary>
    public event Action<byte[]>? Frame;

    /// <summary>Chamado quando a prévia termina sozinha, com a causa (vazia se foi parada).</summary>
    public event Action<string>? Ended;

    private volatile bool _stopping;

    public CameraPreview(string camera, int width, int height)
    {
        (Width, Height) = (width, height);
        _process = Process.Start(Ffmpeg.StartInfo(CaptureArgs.CameraPreview(camera, width, height)))!;
        _stderr = _process.StandardError.ReadToEndAsync();
        _reader = new Thread(Read) { IsBackground = true, Name = "camera-preview" };
        _reader.Start();
    }

    private void Read()
    {
        var frame = new byte[Width * Height * 4];
        var stream = _process.StandardOutput.BaseStream;
        try
        {
            while (true)
            {
                stream.ReadExactly(frame);
                Frame?.Invoke(frame);
            }
        }
        catch (Exception e) when (e is EndOfStreamException or IOException or ObjectDisposedException)
        {
            // FFmpeg encerrou
        }
        _process.WaitForExit(2000);
        _stderr.Wait(2000);
        Ended?.Invoke(_stopping ? "" : FfmpegErrors.Summarize(_stderr.IsCompleted ? _stderr.Result : ""));
    }

    public void Dispose()
    {
        _stopping = true;
        if (!_process.HasExited) _process.Kill(entireProcessTree: true);
        _reader.Join(3000);
        _process.Dispose();
    }
}
