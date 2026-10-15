# GameRec

Gravador de tela para gameplays no Windows, com áudio do jogo e do microfone, detecção automática do jogo aberto e gravação acelerada pela placa de vídeo. Os vídeos saem em MP4 (H.264 + AAC), prontos para subir no YouTube.

![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?logo=windows)
![Rust](https://img.shields.io/badge/Rust-1.92%2B-000000?logo=rust)
![FFmpeg](https://img.shields.io/badge/FFmpeg-8.x-007808?logo=ffmpeg)

## Recursos

- **Detecta o jogo aberto**: grava só a janela do jogo, sem pegar o que estiver por cima (Windows Graphics Capture, funciona com DirectX/Vulkan).
- **F9 em qualquer lugar**: inicia e para a gravação, inclusive com o jogo em tela cheia.
- **Encoder por GPU**: NVIDIA NVENC, AMD AMF ou Intel Quick Sync, detectado automaticamente (com fallback para CPU/x264).
- **Áudio do sistema + microfone**: volumes separados, medidores de nível e sincronia com o vídeo.
- **30 ou 60 FPS**, resolução nativa, 1440p, 1080p ou 720p e quatro níveis de qualidade.
- Também grava monitores inteiros ou qualquer outra janela.
- Lista das gravações recentes e atalho para o YouTube Studio.
- **Leve**: um único `GameRec.exe` de ~5 MB, ~37 MB de RAM, sem Python. A interface é desenhada pela CPU (sem OpenGL/DirectX), então não disputa a placa de vídeo com o jogo, e quase não redesenha enquanto você joga.

## Instalação

1. Baixe o **[GameRec-Setup.exe](https://github.com/eebagabe/windows-rec/releases/latest/download/GameRec-Setup.exe)** (página de [releases](https://github.com/eebagabe/windows-rec/releases)). Os executáveis são assinados digitalmente; veja a [política de assinatura](#code-signing-policy).
2. Abra e siga o assistente: escolha a pasta, se quer atalho na área de trabalho, e clique em **Instalar**.

O instalador já traz o FFmpeg (não precisa de internet durante a instalação), cria os atalhos e coloca o GameRec em **Configurações > Aplicativos instalados** para desinstalar. Não precisa de administrador. Quem tinha a versão em Python pode instalar por cima: a pasta antiga é substituída e as configurações são mantidas. Para atualizar, rode o instalador da versão nova.

> Se aparecer a tela azul **"O Windows protegeu o computador"**, clique em **Mais informações > Executar assim mesmo**.

### Instalação pelo código-fonte

Baixe o projeto (`git clone` ou **Code > Download ZIP**; no ZIP, antes de extrair, clique com o botão direito > **Propriedades** > **Desbloquear**) e dê dois cliques em **`Instalar.bat`**. Esse assistente usa o `GameRec.exe` da pasta do projeto se existir; senão compila com `cargo` (se o Rust estiver instalado) ou baixa da última release. O FFmpeg é baixado na hora, testando versões até achar uma que rode na máquina com encoder de GPU (cerca de 200 MB).

#### Pelo terminal (sem assistente)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
```

Opções:

| Parâmetro | Descrição |
| --- | --- |
| `-InstallDir <pasta>` | Pasta de instalação (padrão: `%LOCALAPPDATA%\Programs\GameRec`). |
| `-ExePath <arquivo>` | Usa um `GameRec.exe` específico em vez de procurar/baixar. |
| `-NoDesktopShortcut` | Não cria atalho na área de trabalho. |
| `-NoStartMenuShortcut` | Não cria atalho no menu Iniciar. |

## Como usar

1. Abra o **GameRec** pelo atalho.
2. Abra o jogo.
3. Dentro do jogo, aperte **F9** para começar e **F9** de novo para parar.

Os vídeos ficam em **`Vídeos\GameRec`** com o nome `Gameplay_AAAA-MM-DD_HH-MM-SS.mp4`. A pasta pode ser trocada no app.

No campo **Gravar** você escolhe:

- **Automático (jogo aberto)**: com F9, grava a janela em foco. Pelo botão, grava o jogo detectado. Sem jogo aberto, grava o monitor principal.
- **Jogo: ... / Janela: ...**: uma janela específica.
- **Tela: Monitor N**: o monitor inteiro.

## Smart App Control

No Windows 11 com o **Smart App Control** ligado, o Windows pode bloquear `.exe` sem assinatura digital paga (principalmente arquivos baixados da internet). O instalador testa se o `GameRec.exe` roda e, se for bloqueado, avisa. Nesse caso compile na própria máquina (`cargo build --release`) ou desligue o Smart App Control em **Segurança do Windows > Controle de aplicativos e navegador**.

## Desinstalar

**Configurações > Aplicativos > Aplicativos instalados > GameRec > Desinstalar**. Os vídeos gravados são mantidos. (Instalações feitas pelo `Instalar.bat` também podem ser removidas com `Desinstalar.bat`.)

## Problemas comuns

| Problema | Solução |
| --- | --- |
| "Jogo minimizado" | Volte para o jogo e aperte F9 dentro dele. |
| Encoder aparece como "CPU (x264)" | Atualize o driver da placa de vídeo. A gravação funciona, mas usa mais processador. |
| Sem som do jogo | Confira se a saída de áudio padrão do Windows é a que o jogo usa (fone/alto-falante). |
| Microfone mudo | Configurações > Privacidade > Microfone > permitir que aplicativos da área de trabalho acessem o microfone. |
| Erro na instalação | O assistente mostra o log. O arquivo completo fica em `%TEMP%\gamerec_setup.log`. |
| Erro ao iniciar a gravação | O app mostra a causa com os botões **Copiar erro** e **Abrir log**. O log fica em `%APPDATA%\GameRec\gamerec.log`. |
| "O Windows bloqueou o FFmpeg" | Smart App Control ou antivírus. Desligue o Smart App Control (Segurança do Windows > Controle de aplicativos e navegador) ou libere a pasta do GameRec no antivírus, e reinstale. |

## Desenvolvimento

```powershell
cargo build --release          # gera target\release\GameRec.exe
# coloque um ffmpeg.exe (8.0+) em target\release\bin\ffmpeg.exe
target\release\GameRec.exe
```

Compilando a partir do Linux/WSL (sem ícone embutido no exe):

```bash
rustup target add x86_64-pc-windows-gnu
cargo install cargo-zigbuild   # precisa do zig no PATH
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

Uma tag `v*` (igual à versão do `Cargo.toml`) dispara o workflow `release`: compila no Windows, baixa o FFmpeg, gera o `GameRec-Setup.exe` com o [Inno Setup](https://jrsoftware.org/isinfo.php) e publica os dois na release.

| Arquivo | Função |
| --- | --- |
| `src/main.rs` | Entrada; cria a janela. |
| `src/window.rs` | Janela winit desenhada por software (softbuffer), com limite de quadros. |
| `src/ui.rs`, `src/theme.rs` | Interface (egui), cores e widgets próprios. |
| `src/controller.rs` | Thread de gravação: inicia/para captura, independente da interface. |
| `src/video.rs` | Comandos do FFmpeg (captura, encoder, mixagem final). |
| `src/audio.rs` | Captura WASAPI (loopback e microfone) em WAV. |
| `src/win.rs` | Win32: monitores, janelas/detecção de jogo, hotkey F9. |
| `src/config.rs` | Configurações em `%APPDATA%\GameRec\config.json`. |
| `src/log.rs` | Log de diagnóstico em `%APPDATA%\GameRec\gamerec.log`. |
| `installer/GameRec.iss` | Instalador (Inno Setup) publicado na release; `make-images.ps1` gera as imagens do assistente. |
| `Instalar.bat` / `scripts/installer.ps1` | Assistente de instalação pelo código-fonte. |
| `scripts/setup.ps1` | Instalação sem interface (usada pelo assistente). |
| `Desinstalar.bat` / `scripts/uninstall.ps1` | Desinstalador. |

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Os executáveis publicados nas [releases](https://github.com/eebagabe/windows-rec/releases) (`GameRec.exe` e `GameRec-Setup.exe`) são compilados pelo GitHub Actions a partir deste repositório público e só são assinados nesse processo. O FFmpeg incluído no instalador é um programa de terceiros ([BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds)) e não é assinado por este projeto.

**Team roles**

- Committers and reviewers: [eebagabe](https://github.com/eebagabe)
- Approvers: [eebagabe](https://github.com/eebagabe)

**Privacy policy**

This program will not transfer any information to other networking systems unless specifically requested by the user or the person installing or operating it. (O GameRec não coleta nem envia dados; o único acesso à internet é abrir o YouTube Studio no navegador quando você clica no botão.)

## Licença

Código do GameRec sob a [licença MIT](LICENSE). O FFmpeg distribuído junto com o instalador tem licença própria (GPL), incluída na instalação em `bin\ffmpeg-LICENSE.txt`.

## Créditos

Gravação feita com [FFmpeg](https://ffmpeg.org) (builds de [BtbN](https://github.com/BtbN/FFmpeg-Builds) e [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), licença GPL), baixado durante a instalação. Interface com [egui](https://github.com/emilk/egui) e [egui_software_backend](https://github.com/DGriffin91/egui_software_backend), áudio com [cpal](https://github.com/RustAudio/cpal).
