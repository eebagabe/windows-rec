using NAudio.CoreAudioApi;
using NAudio.Dmo;
using NAudio.Wave;

namespace WinRec.Audio;

/// <summary>
/// Dispositivos WASAPI. Chamar fora da thread da interface: objetos COM do áudio
/// criados na thread STA do WPF falham quando usados pelas threads de captura (MTA).
/// </summary>
public static class AudioDevices
{
    /// <summary>Saída padrão (gravada em loopback), se houver.</summary>
    public static MMDevice? Output()
    {
        using var en = new MMDeviceEnumerator();
        return en.HasDefaultAudioEndpoint(DataFlow.Render, Role.Console)
            ? en.GetDefaultAudioEndpoint(DataFlow.Render, Role.Console)
            : null;
    }

    /// <summary>Microfones ativos, com o padrão primeiro.</summary>
    public static List<MMDevice> Mics()
    {
        using var en = new MMDeviceEnumerator();
        var defaultId = en.HasDefaultAudioEndpoint(DataFlow.Capture, Role.Console)
            ? en.GetDefaultAudioEndpoint(DataFlow.Capture, Role.Console).ID
            : null;
        return [.. en.EnumerateAudioEndPoints(DataFlow.Capture, DeviceState.Active).OrderBy(d => d.ID != defaultId)];
    }

    /// <summary>Microfone pelo nome; senão o padrão.</summary>
    public static MMDevice? FindMic(string name)
    {
        var mics = Mics();
        return mics.FirstOrDefault(d => d.FriendlyName == name) ?? mics.FirstOrDefault();
    }

    internal static bool IsFloat(WaveFormat f) =>
        f.Encoding == WaveFormatEncoding.IeeeFloat
        || (f is WaveFormatExtensible ext && ext.SubFormat == AudioMediaSubtypes.MEDIASUBTYPE_IEEE_FLOAT);
}
