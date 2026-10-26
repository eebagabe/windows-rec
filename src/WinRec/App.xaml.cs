using System.Windows;
using System.Windows.Threading;
using WinRec.Core;

namespace WinRec;

public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        DispatcherUnhandledException += OnUiException;
        AppDomain.CurrentDomain.UnhandledException += (_, a) => Log.Write($"FALHA: {a.ExceptionObject}");
        base.OnStartup(e);
    }

    private static void OnUiException(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        Log.Write($"FALHA na interface: {e.Exception}");
        MessageBox.Show($"Erro inesperado: {e.Exception.Message}\n\nDetalhes em {AppPaths.LogFile}", "WinRec",
            MessageBoxButton.OK, MessageBoxImage.Error);
        e.Handled = true;
    }
}
