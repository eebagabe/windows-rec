using System.Diagnostics;

namespace WinRec.Core;

/// <summary>Relógio monotônico comum para alinhar áudio e vídeo.</summary>
public static class AppClock
{
    private static readonly long Start = Stopwatch.GetTimestamp();

    /// <summary>Segundos desde o início do app.</summary>
    public static double Now => Stopwatch.GetElapsedTime(Start).TotalSeconds;
}
