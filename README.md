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

1. Baixe o projeto:
   - **Com Git:** `git clone https://github.com/eebagabe/windows-rec.git`
   - **Ou pelo ZIP:** botão verde **Code > Download ZIP**. Antes de extrair, clique com o botão direito no ZIP > **Propriedades** > marque **Desbloquear** > **OK** (assim o Windows não bloqueia os scripts).
2. Dê dois cliques em **`Instalar.bat`**.
3. Siga o assistente: escolha a pasta e os atalhos e clique em **Instalar**.

O assistente baixa e configura tudo sozinho:

| Item | Detalhe |
| --- | --- |
| GameRec.exe | O aplicativo. Usa o `GameRec.exe` da pasta do projeto se existir; senão compila com `cargo` (se o Rust estiver instalado) ou baixa da última [release](https://github.com/eebagabe/windows-rec/releases). |
| FFmpeg | Motor de gravação. O instalador testa versões até achar uma que rode na sua máquina com encoder de GPU. |
| Atalhos | Área de trabalho e menu Iniciar. |

Precisa de internet (cerca de 200 MB, quase tudo FFmpeg). Não precisa de administrador. Quem tinha a versão em Python instalada pode instalar por cima: a pasta antiga é substituída e as configurações são mantidas.

> Se aparecer a tela azul **"O Windows protegeu o computador"**, clique em **Mais informações > Executar assim mesmo**.

### Instalação pelo terminal (sem assistente)

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

**Configurações > Aplicativos > Aplicativos instalados > GameRec > Desinstalar**, ou dois cliques em `Desinstalar.bat`. Os vídeos gravados são mantidos.

## Problemas comuns

| Problema | Solução |
| --- | --- |
| "Jogo minimizado" | Volte para o jogo e aperte F9 dentro dele. |
| Encoder aparece como "CPU (x264)" | Atualize o driver da placa de vídeo. A gravação funciona, mas usa mais processador. |
| Sem som do jogo | Confira se a saída de áudio padrão do Windows é a que o jogo usa (fone/alto-falante). |
| Microfone mudo | Configurações > Privacidade > Microfone > permitir que aplicativos da área de trabalho acessem o microfone. |
| Erro na instalação | O assistente mostra o log. O arquivo completo fica em `%TEMP%\gamerec_setup.log`. |

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

Uma tag `v*` no GitHub dispara o workflow `release`, que compila no Windows e publica o `GameRec.exe` na release (é de lá que o instalador baixa).

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
| `Instalar.bat` / `scripts/installer.ps1` | Assistente gráfico de instalação. |
| `scripts/setup.ps1` | Instalação sem interface (usada pelo assistente). |
| `Desinstalar.bat` / `scripts/uninstall.ps1` | Desinstalador. |

## Créditos

Gravação feita com [FFmpeg](https://ffmpeg.org) (builds de [BtbN](https://github.com/BtbN/FFmpeg-Builds) e [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), licença GPL), baixado durante a instalação. Interface com [egui](https://github.com/emilk/egui) e [egui_software_backend](https://github.com/DGriffin91/egui_software_backend), áudio com [cpal](https://github.com/RustAudio/cpal).
