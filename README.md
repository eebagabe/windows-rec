# GameRec

Gravador de tela para gameplays no Windows, com áudio do jogo e do microfone, detecção automática do jogo aberto e gravação acelerada pela placa de vídeo. Os vídeos saem em MP4 (H.264 + AAC), prontos para subir no YouTube.

![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?logo=windows)
![Python](https://img.shields.io/badge/Python-3.10%2B-3776AB?logo=python&logoColor=white)
![FFmpeg](https://img.shields.io/badge/FFmpeg-8.x-007808?logo=ffmpeg)

## Recursos

- **Detecta o jogo aberto**: grava só a janela do jogo, sem pegar o que estiver por cima (Windows Graphics Capture, funciona com DirectX/Vulkan).
- **F9 em qualquer lugar**: inicia e para a gravação, inclusive com o jogo em tela cheia.
- **Encoder por GPU**: NVIDIA NVENC, AMD AMF ou Intel Quick Sync, detectado automaticamente (com fallback para CPU/x264).
- **Áudio do sistema + microfone**: volumes separados, medidores de nível e sincronia com o vídeo.
- **30 ou 60 FPS**, resolução nativa, 1440p, 1080p ou 720p e quatro níveis de qualidade.
- Também grava monitores inteiros ou qualquer outra janela.
- Lista das gravações recentes e atalho para o YouTube Studio.

## Instalação

1. Baixe o projeto:
   - **Com Git:** `git clone https://github.com/eebagabe/windows-rec.git`
   - **Ou pelo ZIP:** botão verde **Code > Download ZIP**. Antes de extrair, clique com o botão direito no ZIP > **Propriedades** > marque **Desbloquear** > **OK** (assim o Windows não bloqueia os scripts).
2. Dê dois cliques em **`Instalar.bat`**.
3. Siga o assistente: escolha a pasta e os atalhos e clique em **Instalar**.

O assistente baixa e configura tudo sozinho:

| Item | Detalhe |
| --- | --- |
| FFmpeg | Motor de gravação. O instalador testa versões até achar uma que rode na sua máquina com encoder de GPU. |
| Python 3.12 | Só se você ainda não tiver Python 3.10+ (instalação apenas para o seu usuário). |
| Bibliotecas | `customtkinter`, `pyaudiowpatch`, `pillow`. |
| Atalhos | Área de trabalho e menu Iniciar. |

Precisa de internet (cerca de 250 MB). Não precisa de administrador.

> Se aparecer a tela azul **"O Windows protegeu o computador"**, clique em **Mais informações > Executar assim mesmo**.

### Instalação pelo terminal (sem assistente)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
```

Opções:

| Parâmetro | Descrição |
| --- | --- |
| `-InstallDir <pasta>` | Pasta de instalação (padrão: `%LOCALAPPDATA%\Programs\GameRec`). |
| `-Mode auto\|exe\|script` | `exe` compila um `GameRec.exe` com PyInstaller; `script` roda via `pythonw.exe`. Padrão `auto`. |
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

No Windows 11 com o **Smart App Control** ligado, programas `.exe` sem assinatura digital paga são bloqueados. Nesse caso o instalador configura o GameRec para rodar pelo `pythonw.exe` (assinado pela Python Software Foundation): o atalho e o ícone funcionam do mesmo jeito. Com o Smart App Control desligado, o instalador compila um `GameRec.exe`.

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
py -3.12 -m venv .venv
.venv\Scripts\pip install customtkinter pyaudiowpatch pillow
python make_icon.py                     # gera assets\icon.png e assets\icon.ico
# coloque um ffmpeg.exe (8.0+) em bin\ffmpeg.exe
.venv\Scripts\python gamerec.py
```

| Arquivo | Função |
| --- | --- |
| `gamerec.py` | Aplicativo (interface, captura, áudio, mixagem). |
| `make_icon.py` | Gera o ícone. |
| `Instalar.bat` / `scripts/installer.ps1` | Assistente gráfico de instalação. |
| `scripts/setup.ps1` | Instalação sem interface (usada pelo assistente). |
| `Desinstalar.bat` / `scripts/uninstall.ps1` | Desinstalador. |

## Créditos

Gravação feita com [FFmpeg](https://ffmpeg.org) (builds de [BtbN](https://github.com/BtbN/FFmpeg-Builds) e [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), licença GPL), baixado durante a instalação. Interface com [CustomTkinter](https://github.com/TomSchimansky/CustomTkinter) e áudio com [PyAudioWPatch](https://github.com/s0d3s/PyAudioWPatch).
