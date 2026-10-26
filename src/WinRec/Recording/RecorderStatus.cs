namespace WinRec.Recording;

public enum Phase
{
    Idle,
    Starting,
    Recording,
    Saving,
}

public enum Tone
{
    Muted,
    Live,
    Warn,
    Ok,
}

/// <param name="Detail">Explicação completa do último erro (painel com "Copiar").</param>
/// <param name="Warning">Aviso durante a gravação (ex.: microfone falhou, gravando sem ele).</param>
/// <param name="Saved">Incrementa a cada arquivo salvo (a lista de gravações recarrega).</param>
public sealed record RecorderStatus(
    Phase Phase,
    string Headline,
    Tone Tone,
    string Info,
    string? Detail = null,
    string? Warning = null,
    DateTime? StartedAt = null,
    string Target = "",
    bool SystemActive = false,
    bool MicActive = false,
    int Saved = 0);
