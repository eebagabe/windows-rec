using System.Windows.Media;
using CommunityToolkit.Mvvm.ComponentModel;

namespace WinRec.ViewModels;

/// <summary>Vídeo da lista de gravações recentes.</summary>
public sealed partial class RecordingItem(string path, long size, DateTime when) : ObservableObject
{
    public string Path { get; } = path;
    public long Size { get; } = size;
    public string Name { get; } = System.IO.Path.GetFileName(path);

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(Detail))]
    private TimeSpan? _duration;

    [ObservableProperty]
    private ImageSource? _thumbnail;

    public string Detail
    {
        get
        {
            var mb = Size / 1024.0 / 1024.0;
            var size = mb >= 1024 ? $"{mb / 1024:0.0} GB" : $"{mb:0} MB";
            var duration = Duration is { } d ? $"  ·  {(d.TotalHours >= 1 ? d.ToString(@"h\:mm\:ss") : d.ToString(@"mm\:ss"))}" : "";
            return $"{when:dd/MM/yyyy HH:mm}{duration}  ·  {size}";
        }
    }
}

/// <summary>Opção do campo "Gravar".</summary>
public sealed record SourceOption(string Label, string Key, Wpf.Ui.Controls.SymbolRegular Icon);
