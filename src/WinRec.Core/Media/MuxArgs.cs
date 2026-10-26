using System.Globalization;

namespace WinRec.Core.Media;

/// <param name="Offset">Segundos; positivo = o áudio começou antes do vídeo.</param>
/// <param name="Volume">Percentual (100 = original).</param>
public sealed record AudioTrack(string Wav, double Offset, double Volume);

public static class MuxArgs
{
    /// <summary>Junta vídeo + áudios alinhados num MP4 (H.264 copiado + AAC).</summary>
    public static List<string> Build(string video, IReadOnlyList<AudioTrack> audios, string output)
    {
        var inv = CultureInfo.InvariantCulture;
        List<string> a = ["-loglevel", "error", "-y", "-i", video];
        var filters = new List<string>();
        for (var n = 1; n <= audios.Count; n++)
        {
            var t = audios[n - 1];
            a.AddRange(["-i", t.Wav]);
            var chain = $"[{n}:a]";
            if (t.Offset > 0)
                chain += string.Format(inv, "atrim=start={0:F4},asetpts=PTS-STARTPTS,", t.Offset);
            else if (t.Offset < 0)
                chain += string.Format(inv, "adelay={0}:all=1,", (long)(-t.Offset * 1000));
            chain += string.Format(inv, "aresample=48000,aformat=channel_layouts=stereo,volume={0:F2}[a{1}]",
                t.Volume / 100, n);
            filters.Add(chain);
        }
        if (audios.Count == 0)
        {
            a.AddRange(["-map", "0:v"]);
        }
        else
        {
            var outLabel = "[a1]";
            if (audios.Count > 1)
            {
                var labels = string.Concat(Enumerable.Range(1, audios.Count).Select(i => $"[a{i}]"));
                filters.Add($"{labels}amix=inputs={audios.Count}:duration=longest:normalize=0[aout]");
                outLabel = "[aout]";
            }
            a.AddRange(["-filter_complex", string.Join(";", filters), "-map", "0:v", "-map", outLabel,
                "-c:a", "aac", "-b:a", "192k", "-shortest"]);
        }
        a.AddRange(["-c:v", "copy", "-movflags", "+faststart", output]);
        return a;
    }
}
