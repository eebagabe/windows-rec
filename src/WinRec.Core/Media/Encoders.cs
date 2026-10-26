namespace WinRec.Core.Media;

/// <summary>Encoders H.264 suportados e seus parâmetros de qualidade.</summary>
public static class Encoders
{
    public const string Cpu = "libx264";

    /// <summary>Em ordem de preferência.</summary>
    public static readonly string[] Gpu = ["h264_nvenc", "h264_amf", "h264_qsv"];

    public static string DisplayName(string encoder) => encoder switch
    {
        "h264_nvenc" => "NVIDIA NVENC",
        "h264_amf" => "AMD AMF",
        "h264_qsv" => "Intel Quick Sync",
        _ => "CPU (x264)",
    };

    public static string[] Args(string encoder, int quality, int fps)
    {
        var gop = (fps * 2).ToString();
        var q = quality.ToString();
        return encoder switch
        {
            "h264_nvenc" =>
            [
                "-c:v", encoder, "-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", q, "-b:v", "0",
                "-maxrate", "100M", "-bufsize", "200M", "-profile:v", "high", "-g", gop,
            ],
            "h264_amf" =>
            [
                "-c:v", encoder, "-quality", "balanced", "-rc", "cqp", "-qp_i", q, "-qp_p", (quality + 2).ToString(),
                "-profile:v", "high", "-g", gop,
            ],
            "h264_qsv" => ["-c:v", encoder, "-preset", "medium", "-global_quality", q, "-profile:v", "high", "-g", gop],
            _ => ["-c:v", Cpu, "-preset", "veryfast", "-crf", q, "-profile:v", "high", "-g", gop],
        };
    }
}
