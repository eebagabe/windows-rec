using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Win32;
using WinRec.Audio;
using WinRec.Core;
using WinRec.Core.Media;
using WinRec.Interop;
using WinRec.Recording;
using Wpf.Ui.Controls;

namespace WinRec.ViewModels;

public sealed partial class MainViewModel : ObservableObject, IDisposable
{
    private const int MaxRecordings = 8;

    /// <summary>Propriedades que fazem parte da <see cref="AppConfig"/>.</summary>
    private static readonly HashSet<string> ConfigProperties =
    [
        nameof(SelectedSource), nameof(Fps), nameof(Resolution), nameof(Quality), nameof(SystemAudio),
        nameof(SystemVolume), nameof(Mic), nameof(MicDevice), nameof(MicVolume), nameof(OutputDir),
        nameof(MinimizeOnRecord),
    ];

    private readonly RecordingController _controller;
    private readonly DispatcherTimer _tick;
    private readonly DispatcherTimer _saveDebounce;
    private readonly SemaphoreSlim _probeSlots = new(2);
    private readonly Dictionary<(string, long), RecordingItem> _probed = [];
    private AppConfig _config;
    private bool _syncing = true;
    private float _sysPeak, _micPeak;

    public static string Version { get; } = typeof(MainViewModel).Assembly.GetName().Version?.ToString(3) ?? "";

    public MainViewModel()
    {
        _config = AppConfig.Load(AppPaths.ConfigFile);
        Log.Write($"WinRec {Version} iniciado; ffmpeg: {Ffmpeg.ExePath} (existe: {File.Exists(Ffmpeg.ExePath)})");
        _controller = new RecordingController(_config);
        _controller.StatusChanged += s => Application.Current.Dispatcher.BeginInvoke(() => Status = s);
        _status = _controller.Status;

        _fps = _config.Fps;
        _resolution = _config.Resolution;
        _quality = _config.Quality;
        _systemAudio = _config.SystemAudio;
        _systemVolume = _config.SystemVolume;
        _mic = _config.Mic;
        _micDevice = _config.MicDevice;
        _micVolume = _config.MicVolume;
        _outputDir = _config.OutputDir;
        _minimizeOnRecord = _config.MinimizeOnRecord;
        RefreshSources();
        _syncing = false;

        _tick = new DispatcherTimer(DispatcherPriority.Render) { Interval = TimeSpan.FromMilliseconds(50) };
        _tick.Tick += (_, _) => OnTick();
        _saveDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(800) };
        _saveDebounce.Tick += (_, _) =>
        {
            _saveDebounce.Stop();
            _config.Save(AppPaths.ConfigFile);
        };

        RefreshRecordings();
        _ = LoadDevicesAsync();
        _ = DetectEncoderAsync();
    }

    // ------------------------------------------------------------------ estado da gravação

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsIdle), nameof(IsRecording), nameof(IsBusy), nameof(Notice), nameof(ButtonText))]
    private RecorderStatus _status;

    public bool IsIdle => Status.Phase == Phase.Idle;
    public bool IsRecording => Status.Phase == Phase.Recording;
    public bool IsBusy => Status.Phase is Phase.Starting or Phase.Saving;

    public string ButtonText => Status.Phase switch
    {
        Phase.Recording => "PARAR GRAVAÇÃO",
        Phase.Starting => "Iniciando...",
        Phase.Saving => "Salvando vídeo...",
        _ => "INICIAR GRAVAÇÃO",
    };

    /// <summary>Explicação do erro (parado) ou aviso (gravando), mostrada num painel com "Copiar".</summary>
    public string? Notice => Status.Phase switch
    {
        Phase.Idle => Status.Detail,
        Phase.Recording => Status.Warning,
        _ => null,
    };

    [ObservableProperty] private string _elapsed = "00:00:00";
    [ObservableProperty] private string _fpsText = "";
    [ObservableProperty] private double _systemMeter;
    [ObservableProperty] private double _micMeter;
    [ObservableProperty] private string _encoderText = "Detectando encoder...";
    [ObservableProperty] private bool _encoderIsGpu;
    [ObservableProperty] private bool _hotkeyOk = true;
    [ObservableProperty] private string _copyText = "Copiar erro";

    /// <summary>A janela informa; fora de foco o relógio atualiza 1x por segundo.</summary>
    [ObservableProperty] private bool _isWindowActive = true;

    public nint MainWindowHandle
    {
        set => _controller.MainWindow = value;
    }

    partial void OnStatusChanged(RecorderStatus? oldValue, RecorderStatus newValue)
    {
        if (newValue.Saved != oldValue?.Saved) RefreshRecordings();
        UpdateTick();
    }

    partial void OnIsWindowActiveChanged(bool value) => UpdateTick();

    private void OnTick()
    {
        static float Decay(float meter, float level) => Math.Max(level, meter * 0.82f);
        var live = IsRecording;
        _sysPeak = Decay(_sysPeak, live && Status.SystemActive ? _controller.SystemLevel.Take() : 0);
        _micPeak = Decay(_micPeak, live && Status.MicActive ? _controller.MicLevel.Take() : 0);
        SystemMeter = _sysPeak;
        MicMeter = _micPeak;
        var elapsed = Status.StartedAt is { } t ? DateTime.UtcNow - t : TimeSpan.Zero;
        Elapsed = elapsed.ToString(@"hh\:mm\:ss");
        FpsText = live ? $"{_controller.Fps:0} fps" : "";
        UpdateTick();
    }

    /// <summary>Só redesenha enquanto há algo mudando: parado, o app não gasta CPU.</summary>
    private void UpdateTick()
    {
        var needed = !IsIdle || _sysPeak > 0.001 || _micPeak > 0.001;
        if (!needed)
        {
            _tick.Stop();
            return;
        }
        _tick.Interval = IsRecording && !IsWindowActive ? TimeSpan.FromSeconds(1) : TimeSpan.FromMilliseconds(50);
        _tick.Start();
    }

    [RelayCommand]
    private void ToggleRecording() => Toggle(null);

    /// <summary>Do botão (sem janela) ou do F9 (janela em foco no momento).</summary>
    public void Toggle(nint? foreground) => _controller.ToggleRecording(foreground);

    [RelayCommand]
    private async Task CopyErrorAsync()
    {
        Clipboard.SetText($"WinRec {Version} ({Status.Headline})\n{Status.Info}\n\n{Notice}\n\nLog: {AppPaths.LogFile}");
        CopyText = "Copiado!";
        await Task.Delay(2000);
        CopyText = "Copiar erro";
    }

    [RelayCommand]
    private static void OpenLog() => Shell.Open(AppPaths.LogFile);

    private async Task DetectEncoderAsync()
    {
        var enc = await Task.Run(Ffmpeg.DetectEncoderAsync);
        Log.Write($"encoder: {enc}");
        _controller.Encoder = enc;
        EncoderText = Encoders.DisplayName(enc);
        EncoderIsGpu = enc != Encoders.Cpu;
    }

    // ------------------------------------------------------------------ fonte

    public ObservableCollection<SourceOption> Sources { get; } = [];

    [ObservableProperty] private SourceOption? _selectedSource;

    [RelayCommand]
    public void RefreshSources()
    {
        var key = SelectedSource?.Key ?? _config.Source;
        var wasSyncing = _syncing;
        _syncing = true;
        Sources.Clear();
        Sources.Add(new SourceOption("Automático: janela em foco ao apertar F9", SourceKey.Auto, SymbolRegular.Cursor24));
        var monitors = Desktop.Monitors();
        for (var i = 0; i < monitors.Count; i++)
        {
            var m = monitors[i];
            var main = m.Primary ? " (principal)" : "";
            Sources.Add(new SourceOption($"Tela: Monitor {i + 1} · {m.Width}×{m.Height}{main}", SourceKey.Monitor(i), SymbolRegular.Desktop24));
        }
        foreach (var w in Desktop.Windows().DistinctBy(w => w.Exe))
            Sources.Add(new SourceOption($"Janela: {w.Title}", SourceKey.Window(w.Exe), SymbolRegular.WindowApps24));
        // fonte salva que não está disponível agora continua selecionável
        if (Sources.All(s => s.Key != key))
        {
            var (kind, value) = SourceKey.Parse(key);
            if (kind == "win") Sources.Add(new SourceOption($"Janela: {value} (fechada)", key, SymbolRegular.WindowApps24));
        }
        SelectedSource = Sources.FirstOrDefault(s => s.Key == key) ?? Sources[1];
        _syncing = wasSyncing;
        if (!_syncing) ApplyConfig();
    }

    // ------------------------------------------------------------------ vídeo

    public IReadOnlyList<int> FrameRates => AppConfig.FrameRates;
    public IEnumerable<string> Resolutions => AppConfig.Resolutions.Select(r => r.Name);
    public IEnumerable<string> Qualities => AppConfig.Qualities.Select(q => q.Name);

    [ObservableProperty] private int _fps;
    [ObservableProperty] private string _resolution;
    [ObservableProperty] private string _quality;

    // ------------------------------------------------------------------ áudio

    public ObservableCollection<string> MicDevices { get; } = [];

    [ObservableProperty] private bool _hasOutput = true;
    [ObservableProperty] private bool _systemAudio;
    [ObservableProperty] private double _systemVolume;
    [ObservableProperty] private bool _mic;
    [ObservableProperty] private string _micDevice;
    [ObservableProperty] private double _micVolume;

    private async Task LoadDevicesAsync()
    {
        // fora da thread da interface (ver AudioDevices)
        var (hasOutput, mics) = await Task.Run(() => (AudioDevices.Output() is not null,
            AudioDevices.Mics().Select(d => d.FriendlyName).ToList()));
        Log.Write($"áudio: saída={hasOutput} microfones=[{string.Join(", ", mics)}]; pasta={OutputDir}");
        HasOutput = hasOutput;
        if (!hasOutput) SystemAudio = false;
        foreach (var m in mics) MicDevices.Add(m);
        if (mics.Count == 0) Mic = false;
        else if (!mics.Contains(MicDevice)) MicDevice = mics[0];
    }

    // ------------------------------------------------------------------ saída

    [ObservableProperty] private string _outputDir;
    [ObservableProperty] private bool _minimizeOnRecord;

    public ObservableCollection<RecordingItem> Recordings { get; } = [];

    [RelayCommand]
    private void ChooseFolder()
    {
        var dlg = new OpenFolderDialog { InitialDirectory = OutputDir, Title = "Pasta das gravações" };
        if (dlg.ShowDialog() != true) return;
        OutputDir = dlg.FolderName;
        RefreshRecordings();
    }

    [RelayCommand]
    private void OpenFolder()
    {
        Directory.CreateDirectory(OutputDir);
        Shell.Open(OutputDir);
    }

    [RelayCommand]
    private static void OpenRecording(RecordingItem item) => Shell.Open(item.Path);

    [RelayCommand]
    private static void ShowRecording(RecordingItem item) => Shell.ShowInFolder(item.Path);

    private void RefreshRecordings()
    {
        List<FileInfo> files;
        try
        {
            files = [.. new DirectoryInfo(OutputDir).EnumerateFiles("*.mp4")
                .OrderByDescending(f => f.LastWriteTimeUtc).Take(MaxRecordings)];
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or ArgumentException)
        {
            files = [];
        }
        Recordings.Clear();
        foreach (var f in files)
        {
            // miniatura e duração são lidas uma vez por arquivo
            if (!_probed.TryGetValue((f.FullName, f.Length), out var item))
            {
                item = new RecordingItem(f.FullName, f.Length,
                    RecordingName.ParseDate(f.Name) ?? f.LastWriteTime);
                _probed[(f.FullName, f.Length)] = item;
                _ = ProbeAsync(item);
            }
            Recordings.Add(item);
        }
    }

    private async Task ProbeAsync(RecordingItem item)
    {
        await _probeSlots.WaitAsync();
        try
        {
            var (jpeg, duration) = await Task.Run(() => Ffmpeg.ProbeAsync(item.Path, 192));
            item.Duration = duration;
            if (jpeg is null) return;
            var img = new BitmapImage();
            img.BeginInit();
            img.CacheOption = BitmapCacheOption.OnLoad;
            img.StreamSource = new MemoryStream(jpeg);
            img.EndInit();
            img.Freeze();
            item.Thumbnail = img;
        }
        finally
        {
            _probeSlots.Release();
        }
    }

    // ------------------------------------------------------------------ configuração

    protected override void OnPropertyChanged(PropertyChangedEventArgs e)
    {
        base.OnPropertyChanged(e);
        if (!_syncing && e.PropertyName is { } name && ConfigProperties.Contains(name)) ApplyConfig();
    }

    /// <summary>Repassa para a gravação na hora e grava no disco logo depois (sliders mudam muito).</summary>
    private void ApplyConfig()
    {
        _config = _config with
        {
            Source = SelectedSource?.Key ?? _config.Source,
            Fps = Fps,
            Resolution = Resolution,
            Quality = Quality,
            SystemAudio = SystemAudio,
            SystemVolume = Math.Round(SystemVolume),
            Mic = Mic,
            MicDevice = MicDevice,
            MicVolume = Math.Round(MicVolume),
            OutputDir = OutputDir,
            MinimizeOnRecord = MinimizeOnRecord,
        };
        _controller.Config = _config;
        _saveDebounce.Stop();
        _saveDebounce.Start();
    }

    /// <summary>Bloqueia até a gravação em andamento ser salva.</summary>
    public void Dispose()
    {
        _tick.Stop();
        _saveDebounce.Stop();
        _controller.Dispose();
        _config.Save(AppPaths.ConfigFile);
        _probeSlots.Dispose();
    }
}
