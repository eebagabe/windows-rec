using System.Collections.Concurrent;
using System.ComponentModel;
using WinRec.Audio;
using WinRec.Core;
using WinRec.Core.Media;
using WinRec.Interop;

namespace WinRec.Recording;

/// <summary>
/// Thread de gravação: inicia/para a captura independente da interface
/// (o F9 funciona com a janela minimizada e outro app em tela cheia).
/// </summary>
public sealed class RecordingController : IDisposable
{
    private abstract record Command;
    private sealed record Toggle(nint? Foreground) : Command;
    private sealed record Quit : Command;

    private sealed record Session(
        VideoRecorder Video,
        List<(AudioRecorder Recorder, double Volume)> Audios,
        string Tmp,
        string VideoPath,
        string FinalPath);

    private const string SystemWav = "system.wav";
    private const string MicWav = "mic.wav";

    private readonly BlockingCollection<Command> _commands = new();
    private readonly Thread _thread;
    private readonly Lock _gate = new();
    private RecorderStatus _status = new(Phase.Idle, "PRONTO", Tone.Muted, "Pressione F9 em qualquer lugar para iniciar ou parar");

    /// <summary>Configuração usada na próxima gravação (a interface substitui a cada mudança).</summary>
    public volatile AppConfig Config;

    /// <summary>Encoder detectado; nulo enquanto a detecção não termina.</summary>
    public volatile string? Encoder;

    /// <summary>Janela principal, minimizada ao gravar se configurado.</summary>
    public nint MainWindow;

    public AudioLevel SystemLevel { get; } = new();
    public AudioLevel MicLevel { get; } = new();

    /// <summary>FPS real da captura em andamento.</summary>
    public float Fps { get; private set; }

    /// <summary>Disparado na thread de gravação.</summary>
    public event Action<RecorderStatus>? StatusChanged;

    public RecorderStatus Status
    {
        get { lock (_gate) return _status; }
    }

    public RecordingController(AppConfig config)
    {
        Config = config;
        // MTA: os objetos COM do WASAPI são criados e usados aqui
        _thread = new Thread(Run) { Name = "recording", IsBackground = true };
        _thread.SetApartmentState(ApartmentState.MTA);
        _thread.Start();
    }

    /// <summary>Inicia ou para. <paramref name="foreground"/> é a janela em foco quando veio do F9.</summary>
    public void ToggleRecording(nint? foreground = null) => _commands.Add(new Toggle(foreground));

    private void Update(Func<RecorderStatus, RecorderStatus> change)
    {
        RecorderStatus s;
        lock (_gate) s = _status = change(_status);
        StatusChanged?.Invoke(s);
    }

    private void Set(Phase phase, string headline, Tone tone, string info) =>
        Update(s => s with { Phase = phase, Headline = headline, Tone = tone, Info = info, Detail = null });

    private void Fail(string headline, string info, string detail)
    {
        Log.Write($"ERRO {headline}: {info}\n{detail}");
        Update(s => s with { Phase = Phase.Idle, Headline = headline, Tone = Tone.Live, Info = info, Detail = detail });
    }

    private void Run()
    {
        Session? session = null;
        while (true)
        {
            if (!_commands.TryTake(out var cmd, TimeSpan.FromMilliseconds(250)))
            {
                if (session is null) continue;
                if (session.Video.Alive)
                {
                    Fps = session.Video.Fps;
                }
                else
                {
                    // captura caiu (janela fechou etc.): salva o que tiver
                    Stop(session);
                    session = null;
                }
                continue;
            }
            if (cmd is Quit)
            {
                if (session is not null) Stop(session);
                return;
            }
            if (session is null)
            {
                session = Start(((Toggle)cmd).Foreground);
            }
            else
            {
                Stop(session);
                session = null;
            }
            // F9 apertado enquanto iniciava/salvava é ignorado
            while (_commands.TryTake(out var pending))
            {
                if (pending is not Quit) continue;
                if (session is not null) Stop(session);
                return;
            }
        }
    }

    private abstract record Target(string Name);
    private sealed record WindowTarget(WindowInfo Window) : Target(Window.Title);
    private sealed record MonitorTarget(int Index) : Target($"Monitor {Index + 1}");
    private sealed record CameraTarget(string Camera) : Target($"Webcam {Camera}");

    private Target? Resolve(AppConfig cfg, nint? foreground, List<MonitorInfo> monitors)
    {
        var (kind, value) = SourceKey.Parse(cfg.Source);
        switch (kind)
        {
            case "mon":
                return new MonitorTarget(Math.Clamp(int.TryParse(value, out var i) ? i : 0, 0, monitors.Count - 1));
            case "cam":
                return new CameraTarget(value);
            case "win":
                if (Desktop.Windows().FirstOrDefault(w => w.Exe == value) is { } win) return new WindowTarget(win);
                Set(Phase.Idle, "JANELA NÃO ENCONTRADA", Tone.Live,
                    $"{value} não está aberto. Abra o programa ou escolha outra fonte.");
                return null;
            default:
                // automático: janela em foco no F9; pelo botão (o foco é o próprio WinRec), o monitor principal
                return foreground is { } fg && Desktop.Describe(fg) is { } focused
                    ? new WindowTarget(focused)
                    : new MonitorTarget(0);
        }
    }

    private Session? Start(nint? foreground)
    {
        var cfg = Config;
        if (Encoder is not { } encoder)
        {
            Update(s => s with { Info = "Aguarde a detecção do encoder..." });
            return null;
        }
        var monitors = Desktop.Monitors();
        var target = Resolve(cfg, foreground, monitors);
        if (target is null) return null;
        if (target is WindowTarget { Window.Minimized: true } minimized)
        {
            Set(Phase.Idle, "JANELA MINIMIZADA", Tone.Warn,
                $"Abra \"{minimized.Window.Title}\" e aperte F9 com ela em foco para gravar.");
            return null;
        }
        Update(s => s with
        {
            Phase = Phase.Starting, Headline = "INICIANDO", Tone = Tone.Warn, Info = $"Preparando: {target.Name}",
            Detail = null, Warning = null,
        });
        Log.Write($"iniciando: alvo={target} encoder={encoder} fps={cfg.Fps} qualidade={cfg.Quality} "
                  + $"resolucao={cfg.Resolution} sistema={cfg.SystemAudio} mic={cfg.Mic} ({cfg.MicDevice})");

        var stamp = RecordingName.Stamp(DateTime.Now);
        var tmp = Path.Combine(cfg.OutputDir, $".winrec_{stamp}");
        try
        {
            Directory.CreateDirectory(tmp);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            Fail("ERRO NA PASTA", "Não foi possível criar a pasta de vídeos.",
                $"Pasta: {cfg.OutputDir}\nErro: {e.Message}\n\nEscolha outra pasta em Saída > Alterar (pastas do "
                + "OneDrive ou protegidas contra ransomware costumam bloquear).");
            return null;
        }

        // áudio primeiro (o vídeo é alinhado depois pelos timestamps); falha de áudio não impede gravar
        var (audios, warnings) = StartAudio(cfg, tmp);

        var videoPath = Path.Combine(tmp, "video.mkv");
        CaptureSource[] sources = target switch
        {
            WindowTarget => [CaptureSource.GfxWindow, CaptureSource.GdiWindow],
            CameraTarget => [CaptureSource.Camera],
            _ => [CaptureSource.GfxMonitor, CaptureSource.Ddagrab, CaptureSource.GdiDesktop],
        };
        string[] encoders = encoder == Encoders.Cpu ? [encoder] : [encoder, Encoders.Cpu];
        var monIndex = target is MonitorTarget m ? m.Index : 0;
        VideoRecorder? video = null;
        var attempts = new List<string>();
        foreach (var source in sources)
        {
            foreach (var enc in encoders)
            {
                var req = new CaptureRequest(source, enc, cfg.Fps, cfg.MaxHeight, cfg.Crf, videoPath,
                    MonitorIndex: monIndex, Monitor: monitors[monIndex],
                    Window: target is WindowTarget w ? w.Window.Handle : 0,
                    Camera: target is CameraTarget c ? c.Camera : "");
                VideoRecorder v;
                try
                {
                    v = VideoRecorder.Start(CaptureArgs.Build(req));
                }
                catch (Win32Exception e)
                {
                    Discard(audios, tmp);
                    Fail("FFMPEG NÃO ABRIU", "O motor de gravação (FFmpeg) não pôde ser executado.",
                        $"Arquivo: {Ffmpeg.ExePath}\nErro: {e.Message}\n\n{Ffmpeg.SpawnHint(e)}");
                    return null;
                }
                var deadline = DateTime.UtcNow.AddSeconds(source == CaptureSource.Camera ? 8 : 4);
                while (DateTime.UtcNow < deadline && v.Alive && v.Frames == 0) Thread.Sleep(50);
                if (v.Alive && v.Frames > 0)
                {
                    video = v;
                    break;
                }
                v.Kill();
                Log.Write($"falhou {CaptureArgs.Label(source)} + {enc}:\n{v.Stderr.Trim()}");
                var why = FfmpegErrors.Summarize(v.Stderr);
                attempts.Add($"• {CaptureArgs.Label(source)} + {Encoders.DisplayName(enc)}: "
                             + (why.Length > 0 ? why : "nenhum quadro capturado"));
                v.Dispose();
            }
            if (video is not null) break;
        }

        if (video is null)
        {
            Discard(audios, tmp);
            Fail("ERRO NA CAPTURA", $"Não foi possível capturar {target.Name}.",
                $"Tentativas:\n{string.Join('\n', attempts)}\n\nTente gravar outra fonte (ex.: Tela: Monitor 1), "
                + "atualizar o driver da placa de vídeo ou reinstalar o WinRec.");
            return null;
        }
        Log.Write($"gravando ({attempts.Count} tentativa(s) falharam antes)");

        var sysOn = audios.Any(a => Path.GetFileName(a.Recorder.Path) == SystemWav);
        var micOn = audios.Any(a => Path.GetFileName(a.Recorder.Path) == MicWav);
        Update(s => s with
        {
            Phase = Phase.Recording, Headline = "GRAVANDO", Tone = Tone.Live, Info = $"Gravando: {target.Name}",
            StartedAt = DateTime.UtcNow, Target = target.Name, SystemActive = sysOn, MicActive = micOn,
            Warning = warnings.Count > 0 ? string.Join('\n', warnings) : null,
        });
        if (cfg.MinimizeOnRecord) Desktop.Minimize(MainWindow);
        return new Session(video, audios, tmp, videoPath, Path.Combine(cfg.OutputDir, RecordingName.FileName(stamp)));
    }

    private (List<(AudioRecorder Recorder, double Volume)> Audios, List<string> Warnings) StartAudio(
        AppConfig cfg, string tmp)
    {
        var audios = new List<(AudioRecorder Recorder, double Volume)>();
        var warnings = new List<string>();
        if (cfg.SystemAudio)
        {
            try
            {
                if (AudioDevices.Output() is { } output)
                    audios.Add((AudioRecorder.Loopback(output, Path.Combine(tmp, SystemWav), SystemLevel),
                        cfg.SystemVolume));
                else
                    warnings.Add("Som do computador não foi gravado: nenhuma saída de áudio ativa.");
            }
            catch (Exception e)
            {
                warnings.Add($"Som do computador não foi gravado: {e.Message}");
            }
        }
        if (cfg.Mic)
        {
            var mic = AudioDevices.FindMic(cfg.MicDevice);
            try
            {
                if (mic is not null)
                    audios.Add((AudioRecorder.Input(mic, Path.Combine(tmp, MicWav), MicLevel), cfg.MicVolume));
                else
                    warnings.Add("Microfone não foi gravado: nenhum microfone encontrado.");
            }
            catch (Exception e)
            {
                warnings.Add($"Microfone \"{mic?.FriendlyName}\" não foi gravado: {e.Message}. "
                             + "Confira Configurações > Privacidade > Microfone.");
            }
        }
        foreach (var w in warnings) Log.Write($"aviso: {w}");
        return (audios, warnings);
    }

    private static void Discard(List<(AudioRecorder Recorder, double)> audios, string tmp)
    {
        foreach (var (rec, _) in audios) rec.Dispose();
        try
        {
            Directory.Delete(tmp, recursive: true);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            // pasta temporária fica para trás; não atrapalha
        }
    }

    private void Stop(Session s)
    {
        Set(Phase.Saving, "PROCESSANDO", Tone.Warn, "Salvando vídeo...");
        Log.Write("parando");
        s.Video.Stop();
        var v0 = s.Video.StartTime;
        var tracks = new List<AudioTrack>();
        foreach (var (rec, volume) in s.Audios)
        {
            rec.Dispose();
            if (rec.StartTime is { } a0 && v0 is { } start && rec.BytesWritten > 1000)
                tracks.Add(new AudioTrack(rec.Path, start - a0, volume));
        }
        s.Video.Dispose();

        string? error = null;
        try
        {
            var (code, _, err) = Ffmpeg.RunAsync(MuxArgs.Build(s.VideoPath, tracks, s.FinalPath), TimeSpan.FromMinutes(10))
                .GetAwaiter().GetResult();
            if (code != 0)
            {
                Log.Write($"mux falhou ({code}):\n{err}");
                error = FfmpegErrors.Summarize(err) is { Length: > 0 } line ? line : "falha no ffmpeg";
            }
        }
        catch (Win32Exception e)
        {
            error = e.Message;
        }

        if (error is null)
        {
            Log.Write($"salvo: {s.FinalPath}");
            try
            {
                Directory.Delete(s.Tmp, recursive: true);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                // arquivos brutos ficam; o MP4 já está salvo
            }
        }
        else
        {
            Log.Write($"ERRO ao salvar: {error}");
        }
        SystemLevel.Take();
        MicLevel.Take();
        Fps = 0;
        var name = Path.GetFileName(s.FinalPath);
        Update(st => (error is null
            ? st with { Headline = "SALVO", Tone = Tone.Ok, Info = $"Salvo: {name}", Detail = null }
            : st with
            {
                Headline = "ERRO AO SALVAR", Tone = Tone.Live, Info = "O vídeo foi gravado, mas a montagem do MP4 falhou.",
                Detail = $"Erro: {error}\n\nOs arquivos brutos (vídeo e áudio) ficaram em:\n{s.Tmp}",
            }) with
            {
                Phase = Phase.Idle, StartedAt = null, SystemActive = false, MicActive = false, Saved = st.Saved + 1,
                Warning = null,
            });
        Desktop.Restore(MainWindow);
    }

    /// <summary>Fechar no meio da gravação: salva antes de sair (bloqueia até terminar).</summary>
    public void Dispose()
    {
        _commands.Add(new Quit());
        _thread.Join();
        _commands.Dispose();
    }
}
