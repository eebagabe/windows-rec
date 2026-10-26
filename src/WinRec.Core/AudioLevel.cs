namespace WinRec.Core;

/// <summary>Pico de áudio entre leituras da interface (escrito pela thread de captura, lido pela UI).</summary>
public sealed class AudioLevel
{
    private int _bits;

    /// <summary>Guarda o maior pico (0..1) desde a última <see cref="Take"/>.</summary>
    public void Report(float peak)
    {
        // floats positivos comparam igual aos seus bits
        var bits = BitConverter.SingleToInt32Bits(Math.Clamp(peak, 0f, 1f));
        int cur;
        while (bits > (cur = Volatile.Read(ref _bits)) && Interlocked.CompareExchange(ref _bits, bits, cur) != cur)
        {
        }
    }

    public float Take() => BitConverter.Int32BitsToSingle(Interlocked.Exchange(ref _bits, 0));

    /// <summary>Pico absoluto (0..1) de um buffer PCM 16 bits ou float 32 bits.</summary>
    public static float Peak(ReadOnlySpan<byte> data, bool isFloat, int bitsPerSample)
    {
        float peak = 0;
        if (isFloat && bitsPerSample == 32)
        {
            foreach (var s in System.Runtime.InteropServices.MemoryMarshal.Cast<byte, float>(data))
                peak = Math.Max(peak, Math.Abs(s));
        }
        else if (bitsPerSample == 16)
        {
            foreach (var s in System.Runtime.InteropServices.MemoryMarshal.Cast<byte, short>(data))
                peak = Math.Max(peak, Math.Abs(s / 32768f));
        }
        return Math.Min(peak, 1f);
    }
}
