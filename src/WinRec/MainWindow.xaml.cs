using System.ComponentModel;
using System.Windows.Interop;
using WinRec.Interop;
using WinRec.ViewModels;
using Wpf.Ui.Controls;

namespace WinRec;

public partial class MainWindow : FluentWindow
{
    private readonly MainViewModel _vm = new();
    private GlobalHotKey? _hotkey;

    public MainWindow()
    {
        DataContext = _vm;
        InitializeComponent();
        Activated += (_, _) => _vm.IsWindowActive = true;
        Deactivated += (_, _) => _vm.IsWindowActive = false;
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        var handle = new WindowInteropHelper(this).Handle;
        _vm.MainWindowHandle = handle;
        // janela em foco no momento do F9: é o que o modo automático grava
        _hotkey = new GlobalHotKey(handle, GlobalHotKey.F9, () => _vm.Toggle(Desktop.ForegroundWindow()));
        _vm.HotkeyOk = _hotkey.Registered;
    }

    /// <summary>Fechar no meio da gravação: esconde a janela e salva o vídeo antes de sair.</summary>
    protected override void OnClosing(CancelEventArgs e)
    {
        Hide();
        _hotkey?.Dispose();
        _vm.Dispose();
        base.OnClosing(e);
    }

    private void OnSourcesDropDownOpened(object? sender, EventArgs e) => _vm.RefreshSources();
}
