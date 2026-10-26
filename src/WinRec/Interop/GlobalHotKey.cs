using System.Windows.Interop;
using static WinRec.Interop.Native;

namespace WinRec.Interop;

/// <summary>Atalho global (funciona com outro app em foco, inclusive em tela cheia).</summary>
public sealed class GlobalHotKey : IDisposable
{
    public const uint F9 = 0x78;
    private const int Id = 1;

    private readonly HwndSource _source;
    private readonly Action _onPress;

    /// <summary>Falso quando outro programa já usa a tecla.</summary>
    public bool Registered { get; }

    public GlobalHotKey(nint window, uint vk, Action onPress)
    {
        _onPress = onPress;
        _source = HwndSource.FromHwnd(window);
        _source.AddHook(Hook);
        Registered = RegisterHotKey(window, Id, MOD_NOREPEAT, vk);
    }

    private nint Hook(nint hwnd, int msg, nint wParam, nint lParam, ref bool handled)
    {
        if (msg == WM_HOTKEY && wParam == Id)
        {
            _onPress();
            handled = true;
        }
        return 0;
    }

    public void Dispose()
    {
        if (Registered) UnregisterHotKey(_source.Handle, Id);
        _source.RemoveHook(Hook);
    }
}
