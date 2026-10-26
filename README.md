# WinRec

Gravador de tela para Windows: grava monitores, janelas e webcam, com áudio do computador e do microfone, usando a placa de vídeo para codificar. Os vídeos saem em MP4 (H.264 + AAC), prontos para compartilhar ou editar.

![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?logo=windows)
![.NET](https://img.shields.io/badge/.NET-10-512BD4?logo=dotnet)
![C#](https://img.shields.io/badge/C%23-14-239120?logo=csharp)
![FFmpeg](https://img.shields.io/badge/FFmpeg-8.x-007808?logo=ffmpeg)

## Recursos

- **Grava o que você escolher**: um monitor inteiro, uma janela específica (sem pegar o que estiver por cima, via Windows Graphics Capture) ou a webcam.
- **F9 em qualquer lugar**: inicia e para a gravação mesmo com outro programa em tela cheia. No modo automático, o F9 grava a janela que estiver em foco.
- **Encoder por GPU**: NVIDIA NVENC, AMD AMF ou Intel Quick Sync, detectado automaticamente (com fallback para CPU/x264).
- **Áudio do computador + microfone**: volumes separados, medidores de nível e sincronia com o vídeo.
- **Teste de microfone**: grava 5 segundos com medidor ao vivo e toca de volta para você ouvir.
- **Teste de webcam**: prévia ao vivo para conferir enquadramento e iluminação antes de gravar.
- **30 ou 60 FPS**, resolução nativa, 1440p, 1080p ou 720p e quatro níveis de qualidade.
- Lista das gravações recentes com miniatura, duração e tamanho.
- **Leve**: a interface só redesenha quando algo muda; parado, o app não gasta CPU. Durante a gravação, fora de foco, atualiza uma vez por segundo.

## Instalação

1. Baixe o **[WinRec-Setup.exe](https://github.com/eebagabe/windows-rec/releases/latest/download/WinRec-Setup.exe)** (página de [releases](https://github.com/eebagabe/windows-rec/releases)). Os executáveis são assinados digitalmente; veja a [política de assinatura](#code-signing-policy).
2. Abra e siga o assistente: escolha a pasta, se quer atalho na área de trabalho, e clique em **Instalar**.

O instalador já traz o FFmpeg e tudo o que o app precisa (não é necessário instalar o .NET), cria os atalhos e coloca o WinRec em **Configurações > Aplicativos instalados** para desinstalar. Não precisa de administrador. Para atualizar, rode o instalador da versão nova.

> Se aparecer a tela azul **"O Windows protegeu o computador"**, clique em **Mais informações > Executar assim mesmo**.

## Como usar

1. Abra o **WinRec** pelo atalho.
2. Em **O que gravar**, escolha o monitor, a janela ou a webcam.
3. Aperte **F9** (ou o botão **Iniciar gravação**) para começar e **F9** de novo para parar.

Os vídeos ficam em **`Vídeos\WinRec`** com o nome `Gravacao_AAAA-MM-DD_HH-MM-SS.mp4`. A pasta pode ser trocada em **Saída > Alterar**.

Opções do campo **O que gravar**:

- **Automático**: com F9, grava a janela em foco. Pelo botão, grava o monitor principal.
- **Tela: Monitor N**: o monitor inteiro.
- **Webcam: ...**: a câmera escolhida.
- **Janela: ...**: um programa específico, mesmo que outras janelas fiquem por cima.

## Problemas comuns

| Problema | Solução |
| --- | --- |
| "Janela minimizada" | Restaure a janela e aperte F9 com ela em foco. |
| Encoder aparece como "CPU (x264)" | Atualize o driver da placa de vídeo. A gravação funciona, mas usa mais processador. |
| Sem som do computador | Confira se a saída de áudio padrão do Windows é a que o programa usa (fone/alto-falante). |
| Microfone mudo | Use **Testar microfone**. Se nada for captado: Configurações > Privacidade > Microfone > permitir que aplicativos da área de trabalho acessem o microfone. |
| Webcam não abre | Feche outros programas que estejam usando a câmera (Teams, Zoom, navegador) e clique em **Testar câmera** de novo. |
| Erro ao iniciar a gravação | O app mostra a causa com os botões **Copiar erro** e **Abrir log**. O log fica em `%APPDATA%\WinRec\winrec.log`. |
| "O Windows bloqueou o FFmpeg" | Smart App Control ou antivírus. Desligue o Smart App Control (Segurança do Windows > Controle de aplicativos e navegador) ou libere a pasta do WinRec no antivírus, e reinstale. |

## Desinstalar

**Configurações > Aplicativos > Aplicativos instalados > WinRec > Desinstalar**. Os vídeos gravados são mantidos.

## Desenvolvimento

Requisitos: [.NET SDK 10](https://dotnet.microsoft.com/download) e, para gerar o instalador, [Inno Setup 6.7+](https://jrsoftware.org/isinfo.php).

```powershell
dotnet test                       # testes da biblioteca (rodam também em Linux/macOS)
dotnet run --project src\WinRec   # precisa de um ffmpeg.exe 8.0+ no PATH (ou em bin\ ao lado do WinRec.exe)
.\build.ps1                       # testes + publish + FFmpeg + instalador em dist\WinRec-Setup.exe
```

O FFmpeg usado no instalador pode ser baixado separadamente com `scripts\get-ffmpeg.ps1` (vai para `build\ffmpeg`).

O workflow `release` roda os testes e gera o instalador a cada push e pull request. Uma tag `v*` (igual à `<Version>` do `Directory.Build.props`) publica o `WinRec-Setup.exe` na release.

### Arquitetura

| Projeto / pasta | Função |
| --- | --- |
| `src/WinRec.Core` | Biblioteca sem dependência de interface: configuração, log, linhas de comando do FFmpeg (captura, encoder, mixagem), detecção de encoder, prévia da webcam e medidor de áudio. |
| `src/WinRec/Recording` | `RecordingController`: thread de gravação independente da interface; resolve o alvo, tenta fontes de captura e encoders em cascata e junta vídeo e áudio alinhados no final. |
| `src/WinRec/Audio` | Captura WASAPI (loopback do sistema e microfone) com NAudio e o teste de microfone. |
| `src/WinRec/Interop` | Win32: monitores, janelas e atalho global F9. |
| `src/WinRec/ViewModels`, `MainWindow.xaml` | Interface WPF em MVVM (CommunityToolkit.Mvvm) com visual Fluent (WPF-UI). |
| `tests/WinRec.Core.Tests` | Testes xUnit da biblioteca. |
| `installer/WinRec.iss` | Assistente de instalação (Inno Setup); `make-images.ps1` gera as imagens do assistente. |

Como a gravação funciona: o vídeo é capturado pelo FFmpeg (WGC, DXGI ou GDI, nessa ordem de preferência) direto para um MKV temporário, enquanto o áudio é gravado em WAV pelo WASAPI. Ao parar, o FFmpeg junta tudo num MP4, alinhando cada faixa de áudio pelo instante do primeiro quadro, sem recodificar o vídeo.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Os executáveis publicados nas [releases](https://github.com/eebagabe/windows-rec/releases) (`WinRec.exe` e `WinRec-Setup.exe`) são compilados pelo GitHub Actions a partir deste repositório público e só são assinados nesse processo. O FFmpeg incluído no instalador é um programa de terceiros ([BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds)) e não é assinado por este projeto.

**Team roles**

- Committers and reviewers: [eebagabe](https://github.com/eebagabe)
- Approvers: [eebagabe](https://github.com/eebagabe)

**Privacy policy**

This program will not transfer any information to other networking systems unless specifically requested by the user or the person installing or operating it. (O WinRec não coleta nem envia dados e não acessa a internet.)

## Licença

Código do WinRec sob a [licença MIT](LICENSE). O FFmpeg distribuído junto com o instalador tem licença própria (GPL), incluída na instalação em `bin\ffmpeg-LICENSE.txt`.

## Créditos

Gravação feita com [FFmpeg](https://ffmpeg.org) (builds de [BtbN](https://github.com/BtbN/FFmpeg-Builds), licença GPL). Interface com [WPF-UI](https://github.com/lepoco/wpfui) e [CommunityToolkit.Mvvm](https://github.com/CommunityToolkit/dotnet), áudio com [NAudio](https://github.com/naudio/NAudio).
