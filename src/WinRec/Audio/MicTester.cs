using NAudio.CoreAudioApi;
using NAudio.Wave;
using NAudio.Wave.SampleProviders;
using WinRec.Core;

namespace WinRec.Audio;

/// <summary>Teste de microfone: grava alguns segundos e toca de volta na saída padrão.</summary>
public static class MicTester
{
    /// <summary>Chamar fora da thread da interface (ver <see cref="AudioDevices"/>).</summary>
    /// <returns>Maior pico captado (0..1).</returns>
    public static async Task<float> RecordAndPlayAsync(string micName, TimeSpan duration, AudioLevel level,
        IProgress<string> phase, CancellationToken ct)
    {
        using var dev = AudioDevices.FindMic(micName)
                        ?? throw new InvalidOperationException("Nenhum microfone encontrado.");
        using var capture = new WasapiCapture(dev);
        var fmt = capture.WaveFormat;
        var isFloat = AudioDevices.IsFloat(fmt);
        var buffer = new MemoryStream();
        float max = 0;
        var stopped = new TaskCompletionSource<Exception?>(TaskCreationOptions.RunContinuationsAsynchronously);
        capture.DataAvailable += (_, e) =>
        {
            buffer.Write(e.Buffer, 0, e.BytesRecorded);
            var peak = AudioLevel.Peak(e.Buffer.AsSpan(0, e.BytesRecorded), isFloat, fmt.BitsPerSample);
            level.Report(peak);
            max = Math.Max(max, peak);
        };
        capture.RecordingStopped += (_, e) => stopped.TrySetResult(e.Exception);

        phase.Report($"Fale algo... gravando {duration.TotalSeconds:0} segundos");
        capture.StartRecording();
        try
        {
            await Task.Delay(duration, ct);
        }
        finally
        {
            capture.StopRecording();
            await stopped.Task;
        }
        if (await stopped.Task is { } error) throw error;

        phase.Report("Reproduzindo o que foi gravado...");
        buffer.Position = 0;
        // mesmo layout de bytes, mas no formato padrão que o NAudio sabe converter
        var plain = isFloat
            ? WaveFormat.CreateIeeeFloatWaveFormat(fmt.SampleRate, fmt.Channels)
            : new WaveFormat(fmt.SampleRate, fmt.BitsPerSample, fmt.Channels);
        using var raw = new RawSourceWaveStream(buffer, plain);
        var meter = new MeteringSampleProvider(raw.ToSampleProvider(), fmt.SampleRate / 20);
        meter.StreamVolume += (_, e) => level.Report(e.MaxSampleValues.Max());
        ISampleProvider output = fmt.Channels == 1 ? meter.ToStereo() : meter;

        using var player = new WasapiOut(AudioClientShareMode.Shared, 100);
        var done = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        player.PlaybackStopped += (_, _) => done.TrySetResult();
        player.Init(output);
        player.Play();
        await using (ct.Register(player.Stop))
        {
            await done.Task;
        }
        return max;
    }
}
