using NAudio.CoreAudioApi;
using NAudio.Wave;
using WinRec.Core;

namespace WinRec.Audio;

/// <summary>Grava um dispositivo WASAPI (microfone ou loopback da saída) num WAV, no formato nativo.</summary>
public sealed class AudioRecorder : IDisposable
{
    private readonly WasapiCapture _capture;
    private readonly WaveFileWriter _writer;
    private readonly WasapiOut? _keepalive;
    private readonly ManualResetEventSlim _stopped = new();
    private readonly AudioLevel _level;
    private readonly bool _float;
    private double _start = double.NaN;
    private long _bytes;

    public string Path { get; }

    /// <summary>Instante (relógio do app) do primeiro sample capturado.</summary>
    public double? StartTime => double.IsNaN(_start) ? null : _start;

    public long BytesWritten => Interlocked.Read(ref _bytes);

    private AudioRecorder(WasapiCapture capture, string path, AudioLevel level, WasapiOut? keepalive)
    {
        (_capture, Path, _level, _keepalive) = (capture, path, level, keepalive);
        _float = AudioDevices.IsFloat(capture.WaveFormat);
        _writer = new WaveFileWriter(path, capture.WaveFormat);
        _capture.DataAvailable += OnData;
        _capture.RecordingStopped += (_, e) =>
        {
            if (e.Exception is { } ex) Log.Write($"áudio parou com erro: {ex.Message}");
            _stopped.Set();
        };
        _capture.StartRecording();
    }

    public static AudioRecorder Input(MMDevice dev, string path, AudioLevel level) =>
        new(new WasapiCapture(dev), path, level, null);

    /// <summary>
    /// Loopback do que toca na saída. O WASAPI não entrega dados em silêncio,
    /// então toca silêncio na mesma saída para manter o fluxo contínuo (e o áudio alinhado).
    /// </summary>
    public static AudioRecorder Loopback(MMDevice dev, string path, AudioLevel level)
    {
        WasapiOut? keepalive = null;
        try
        {
            keepalive = new WasapiOut(dev, AudioClientShareMode.Shared, true, 200);
            keepalive.Init(new SilenceProvider(dev.AudioClient.MixFormat));
            keepalive.Play();
        }
        catch (Exception e) when (e is System.Runtime.InteropServices.COMException or InvalidOperationException)
        {
            Log.Write($"keepalive de áudio indisponível: {e.Message}");
            keepalive?.Dispose();
            keepalive = null;
        }
        return new(new WasapiLoopbackCapture(dev), path, level, keepalive);
    }

    private void OnData(object? sender, WaveInEventArgs e)
    {
        if (e.BytesRecorded == 0) return;
        if (double.IsNaN(_start))
        {
            var fmt = _capture.WaveFormat;
            _start = AppClock.Now - (double)e.BytesRecorded / fmt.AverageBytesPerSecond;
        }
        var data = e.Buffer.AsSpan(0, e.BytesRecorded);
        _level.Report(AudioLevel.Peak(data, _float, _capture.WaveFormat.BitsPerSample));
        _writer.Write(e.Buffer, 0, e.BytesRecorded);
        Interlocked.Add(ref _bytes, e.BytesRecorded);
    }

    /// <summary>Para a captura e fecha o WAV.</summary>
    public void Stop()
    {
        if (_capture.CaptureState != CaptureState.Stopped)
        {
            _capture.StopRecording();
            _stopped.Wait(TimeSpan.FromSeconds(5));
        }
        _keepalive?.Stop();
        _writer.Dispose();
    }

    public void Dispose()
    {
        Stop();
        _capture.Dispose();
        _keepalive?.Dispose();
        _stopped.Dispose();
    }
}
