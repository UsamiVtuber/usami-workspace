# Usami Desktop

Aplicativo nativo da companion Usami para Windows, escrito em **Rust** sobre o
**GPUI 0.2.2** (framework de UI da Zed Industries). Abre uma janela com o palco
VRM 3D, chat com o backend local, TTS com lipsync e um modo chamada mãos-livres
com microfone, sem navegador embutido.

- **Versão atual:** 0.5.1 (`Cargo.toml`, aba SOBRE do painel).
- **Canal de releases:** [`UsamiVtuber/usami-desktop`](https://github.com/UsamiVtuber/usami-desktop) (público).
- **Distribuição:** zip Windows x64 com o executável e a pasta `assets/`. O
  pacote **não** inclui modelos `.vrm` (ver [Modelo e assets](#modelo-e-assets)).

## O que o app faz

- Renderiza o avatar VRM em um palco interativo (arrastar orbita; a rolagem
  aproxima e afasta).
- Conversa com a API local (`/api/chat`) e exibe as mensagens em balão e no
  histórico flutuante.
- Toca a resposta com a voz configurada no backend (`/api/tts`), com streaming
  PCM para começar a falar antes do áudio terminar e lipsync pelo volume.
- Em chamada, ouve o microfone em contínuo, segmenta a fala por silêncio e
  transcreve (`/api/stt`) sem clique.
- Oferece um painel de configurações com as seis abas do app web (CONVERSA,
  VOZ, AVATAR, MEMÓRIA, JEITO e SOBRE), com os controles que dependem do web
  apresentados como referência desabilitada.
- Tem modo companhia (ícone da lua): emoção `sleepy` e status `MODO COMPANION`
  por até 30 minutos.
- Lembra a conversa entre sessões: as últimas 100 mensagens, o mudo e a
  geometria da janela ficam em `%APPDATA%\Usami`.
- Sente a ausência de imediato: ao minimizar, esconder na bandeja ou perder o
  foco o status vira `AUSENTE`; ao voltar ela cumprimenta de acordo com o tempo
  fora, e detecta o fechamento abrupto da sessão anterior. Com a janela em
  primeiro plano e sem interação, ela puxa assunto aos ~3,5, 7 e 10 minutos.
- Segura o fechamento para se despedir: o X, o Alt+F4 e o "Sair" da bandeja
  pedem a fala de despedida e a janela só fecha quando o balão aparece e o
  áudio termina (no mudo, ~1,2 s), com teto de 10 s. Fechar de novo durante a
  despedida sai na hora.
- Mostra o vínculo da VN a partir do core local (`GET /api/bond`): nível e EXP
  no pill do topo, medidores de carinho/humor/companhia, resumo e histórico de
  momentos no painel. Cada mensagem enviada rende XP pelas mesmas regras do web
  (`POST /api/bond/event`, com `spoken` em chamada) e encerrar uma chamada rende
  companhia por minuto inteiro. São cinco níveis, até "Almas Gêmeas".

### Persistência local

| Arquivo | Conteúdo |
| --- | --- |
| `conversation.json` | últimas 100 mensagens (texto e emoção). |
| `ui.json` | mudo e posição/tamanho da janela (descartada se a geometria for inválida). |
| `session.json` | início, batimento a cada 60 s e `clean_shutdown` — é o que permite notar um kill abrupto na próxima abertura. |

As gravações usam arquivo temporário + rename para não deixar JSON truncado se
o processo morrer no meio. O vínculo (medidores de afeto) não mora aqui: ele é
compartilhado por dispositivo e pertence ao core local (`usami/data/bond.json`),
lido e atualizado via `/api/bond` e `/api/bond/event`.

### Por que nativo

O app renderiza com a GPU pela stack do GPUI/WGPU, toca PCM direto em
`cpal`/`rodio` (sem camada de mídia do navegador) e mantém a UI em Rust. Isso
evita embarcar um Chromium e dá controle do áudio por amostra, necessário para
o streaming de TTS, o lipsync e a VAD da chamada.

## Requisitos

- **Windows 10/11 x64.** O pacote publicado é só para essa plataforma.
- **Rust** com suporte a `edition 2024` para compilar (toolchain atual; testado
  com a estável recente).
- **Backend da Usami** em `http://127.0.0.1:8787` — `cd usami; npm run api`.
  Sem ele a janela abre, mas chat, TTS e STT não respondem.
- Um modelo `.vrm` próprio e, opcionalmente, clipes `.vrma`. Sem modelo a cena
  abre sem avatar.

## Como rodar (desenvolvimento)

```powershell
# terminal 1 — backend local
cd usami
npm run api

# terminal 2 — app
cd usami-desktop
cargo run
```

Em desenvolvimento os assets são resolvidos a partir da pasta da crate
(`CARGO_MANIFEST_DIR`), então o app roda de qualquer diretório de trabalho. O
`.vrm` de desenvolvimento é buscado em `../usami/public/models/shiro.vrm` e as
animações em `../usami/public/animations/vrma` (ver
[Modelo e assets](#modelo-e-assets)).

## Arquitetura

| Caminho | Responsabilidade |
| --- | --- |
| `src/main.rs` | Janela e estado do app, palco 3D, chat, balão de fala, campo de composição, dock, histórico, painel de configurações e orquestração do fluxo de conversa/chamada. |
| `src/theme.rs` | Tokens de tema (paleta escura, tipografia, espaçamento, raios), fontes embutidas e `glow`. |
| `src/api.rs` | Cliente HTTP (`reqwest`) para `/api/chat`, `/api/tts` e `/api/stt`; timeouts e parsing do streaming de TTS. |
| `src/audio.rs` | Saída de áudio com `cpal`/`rodio`; fila de PCM contínua, sem pausas entre trechos. |
| `src/recorder.rs` | Captura do microfone em thread própria (`cpal`), VAD por frames de 20 ms, `MicGate` e codificação WAV PCM 16-bit mono. |
| `src/storage.rs` | Persistência local em `%APPDATA%\Usami`: conversa, preferências (mudo e geometria da janela) e marcador de sessão com batimento. |
| `../gpui-vrm` | Crate local do motor VRM/glTF/VRMA (WGPU, MToon cel-shading, SpringBone, blendshapes, lipsync e look-at IK). |
| `PACKAGE_README.md` | README que acompanha o zip de release (instalação, requisitos, aviso do SmartScreen). |
| `scripts/release.ps1` | Build e publicação local da release no repo público. |

Principais dependências: `gpui` 0.2.2, `cpal` 0.18.2, `rodio` 0.23.0,
`reqwest` 0.13.5 (`json`, `multipart`, `stream`), `tokio` 1.54.1, `serde` e
`serde_json`.

## Interface

- **Palco:** avatar VRM com fundo configurável; arrastar orbita, a rolagem
  aproxima/afasta e o botão de recentralizar volta ao enquadramento padrão.
  No topo ficam também as animações (`clapperboard`) e a troca de fundo.
- **Balão de fala:** mostra a última resposta da Usami, com destaque tipográfico.
- **Chat:** campo de texto com histórico flutuante e botão de enviar; o dock
  também abre o histórico.
- **Dock durante a chamada:** escuta (pulse), histórico, encerrar, mudo e modo
  companhia (lua).
- **Barra de status:** estado da conversa e da chamada (`COM VOCÊ`, `OUVINDO`,
  `SUA VEZ`, `MODO COMPANION`, `AUSENTE`), com LED que pulsa enquanto ela pensa.
  `AUSENTE` aparece quando a janela está minimizada, escondida na bandeja ou
  sem foco (chamada e modo companhia são exceções).

### Painel de configurações

A barra de abas replica o `.panel-tabs` do web. Como no desktop não há rolagem
horizontal no painel, as seis abas quebram em **duas linhas** com o mesmo
visual (pill selecionado com borda rosa e brilho).

| Aba | Conteúdo | Estado no desktop |
| --- | --- | --- |
| CONVERSA | status do motor, backend usado e modelo carregado. | leitura |
| VOZ | voz oficial, tags de som (`<laugh>`, `<chuckle>`, `<sigh>`, `<short pause>`), privacidade e variáveis do `.env`. | leitura |
| AVATAR | cenário de fundo, modelo e inspeção do rig. | leitura (controles no web) |
| MEMÓRIA | resumo do que foi guardado, campo `APAGAR` e exclusões. | leitura (dados no web/backend) |
| JEITO | sliders de traços e instruções extras. | leitura (personalidade no web) |
| SOBRE | versão, tecnologias, créditos e privacidade. | leitura |

Os controles desabilitados são intencionais: o app desktop não lê nem grava
memória, personalidade ou rig. Esses ajustes continuam no app web e no
`memory.json` do servidor local.

## Tema e tipografia

O tema escuro usa a paleta rosa/perolado da Usami (`#FFA6C9`, `#F9F9FB`,
`#FF5E7E`) definida em `src/theme.rs`, com espaçamento e raios compartilhados
pela UI. As fontes são **embutidas** e carregadas de `assets/fonts/`, sem
download em tempo de execução:

- **DM Sans** — texto de interface.
- **Fraunces** — títulos editoriais, fala da personagem e badge de vínculo.
- **Rajdhani** — rótulos de display (status, níveis, cabeçalhos).

Todas estão sob a SIL Open Font License 1.1. Autores, arquivos e textos das
licenças em [`docs/CREDITS.md`](docs/CREDITS.md).

## Modelo e assets

O app resolve assets ao lado do executável: se existir uma pasta `assets/` na
mesma pasta do `.exe` (caso do pacote), ela é a base; caso contrário usa a pasta
da crate (desenvolvimento). O arquivo `.vrm` **não** é distribuído — por
licença, o usuário aponta um modelo próprio.

A variável de ambiente, quando definida, vence; sem ela o app tenta os
candidatos na ordem abaixo, do primeiro para o último existente:

| Asset | Variável | Candidatos |
| --- | --- | --- |
| Modelo | `USAMI_VRM_PATH` | `../usami/public/models/shiro.vrm` → `assets/shiro.vrm` |
| Animações | `USAMI_VRMA_DIR` | `../usami/public/animations/vrma` → `assets/vrma` |
| Fundo | `USAMI_BACKGROUND` | `assets/background.png` → `../usami/public/background.png` |

No pacote, o usuário pode colocar o modelo em `assets/shiro.vrm` e os clipes em
`assets/vrma/` dentro da pasta extraída — ou apontar os caminhos pelas variáveis
acima.

## Variáveis de ambiente

| Variável | Padrão | Descrição |
| --- | --- | --- |
| `USAMI_API_BASE` | `http://127.0.0.1:8787` | Base do backend. Espaços em branco são ignorados. |
| `USAMI_VRM_PATH` | ver tabela acima | Arquivo `.vrm` do avatar. |
| `USAMI_VRMA_DIR` | ver tabela acima | Pasta de clipes `.vrma`. |
| `USAMI_BACKGROUND` | `assets/background.png` | Imagem de fundo da janela. |

O cliente HTTP usa **5 s** para conectar e **90 s** para a requisição completa;
o TTS em streaming sobe esse teto para **180 s** para não cortar falas longas.

## API local

O app fala apenas com a API local; nenhuma chave de provedor fica no executável.

| Endpoint | Uso | Contrato |
| --- | --- | --- |
| `POST /api/chat` | enviar a mensagem e receber a resposta | JSON com `message`, `capabilities`, `wake_up_jolt` e `sleep_companion_active` |
| `POST /api/tts` | sintetizar a resposta | JSON `{text, style_id, stream:true}`; resposta PCM crua (com sample rate no header) ou áudio completo |
| `POST /api/stt` | transcrever a fala da chamada | multipart, campo `audio` (WAV); resposta `{"text": "..."}` |
| `GET /api/bond` | ler o vínculo do core | JSON `{ok, bond}` com `intimacy`, `meters` (xp, nível e progresso por medidor), `log`, `updatedAt` e `summary` |
| `POST /api/bond/event` | render XP por resposta ou chamada | JSON `{content, spoken}` ou `{kind:"call", minutes}`; devolve o vínculo atualizado |
| `GET /api/proactive/events` | buscar eventos proativos | header `x-usami-activity` com `active`, `idle` ou `away`; em `away`, conversa social é confirmada como `skipped` e lembrete fica pendente |
| `POST /api/proactive/events/ack` | confirmar entrega | JSON `{acks:[{id,outcome}]}` |

O backend precisa expor esses endpoints (ver `usami/` e o README da raiz).
Erros HTTP aparecem no status; sem microfone, o app mostra o erro e continua
funcionando pelo campo de texto.

## Modo chamada (handsfree)

Entrar em chamada abre o microfone automaticamente. A fala é segmentada no
próprio app, em frames de 20 ms:

1. **400 ms de pré-roll** guardam o início da frase; **120 ms de voz** (6 frames)
   abrem a fala;
2. **900 ms de silêncio** (45 frames) encerram o turno; falas menores que
   **350 ms** são descartadas;
3. cada trecho vira WAV PCM 16-bit mono, com teto de **30 s**, e vai para
   `/api/stt`;
4. o texto transcrito é enviado ao `/api/chat` sem clique. Se a Usami ainda
   estiver pensando, o texto fica no campo em vez de falar por cima.

O `MicGate` fecha a captura enquanto a Usami pensa ou fala (a própria voz não
vira transcrição), ao mutar e quando a escuta está desligada. Encerrar a
chamada desliga escuta, áudio e microfone.

## Build e testes

```powershell
cd usami-desktop
cargo build --release --locked   # build de release para o pacote
cargo test                       # 43 testes
cargo clippy                     # sem warnings novos
```

A suíte cobre os contratos de `/api/chat`, `/api/tts` (streaming e fallback),
`/api/stt` e `/api/bond` (leitura e eventos de chat/chamada) contra um servidor
HTTP local, erros HTTP, normalização da base, codificação WAV, VAD e o cadeado
do microfone, além da resolução de caminhos de assets e ícones.

## Release

A publicação é **local e manual**, sem PAT ou secret em CI: o script usa o `gh`
já autenticado na máquina de quem publica.

```powershell
cd usami-desktop
gh auth login                          # uma vez, se ainda não estiver logado
./scripts/release.ps1 -Version 0.5.1   # próxima versão: build + zip + release
./scripts/release.ps1 -Version 0.5.1 -DryRun   # só monta o pacote, não publica
```

Use uma versão ainda não publicada: o script aborta se a release já existir (a
v0.5.1 é a atual).

O que o script faz:

1. valida a versão (`X.Y.Z`) e, fora do `-DryRun`, confere `gh auth status` e se
   a release `v<versão>` já existe (aborta se existir);
2. roda `cargo build --release --locked`;
3. monta `dist/usami-desktop-v<versão>-windows-x64/` com o `.exe`, a pasta
   `assets/` e `PACKAGE_README.md` renomeado para `README.md`;
4. gera `dist/usami-desktop-v<versão>-windows-x64.zip`;
5. publica com `gh release create` no repo `UsamiVtuber/usami-desktop`, título
   `Usami Desktop <versão>` e notas curtas sobre o backend local e o `.vrm`.

O zip **não inclui** `.vrm`, `.glb` ou `.fbx`. A release **v0.5.1** tem
12.840.037 bytes (~12,2 MiB) e foi publicada em 2026-11-11.

Notas de manutenção do script (PowerShell 5.1):

- o `.ps1` é salvo em **UTF-8 com BOM**; sem BOM o PS 5.1 lê como ANSI e
  corrompe acentos;
- as notas são escritas em UTF-8 e passadas com `--notes-file`, sem depender do
  code page do console;
- o “release not found” do `gh` vai para stderr e, com
  `$ErrorActionPreference='Stop'`, viraria erro terminal: a checagem roda em
  modo tolerante e decide por `$LASTEXITCODE`;
- ao mudar a versão ou dependências, rode antes um `cargo build` sem `--locked`
  para atualizar o `Cargo.lock` (o release usa `--locked`).

## Limitações conhecidas

- O executável **não é assinado**; o Windows pode exibir “O Windows protegeu o
  PC” (use **Mais informações → Executar assim mesmo**).
- O pacote é só para **Windows x64**.
- O modelo `.vrm` e os clipes `.vrma` não acompanham o zip.
- O backend precisa estar no ar; não há modo offline de chat/TTS/STT.
- O painel do desktop é visual para as abas AVATAR, MEMÓRIA e JEITO: rig,
  memória e personalidade são editados no app web/backend.
- O pacote exige que `assets/` fique ao lado do `usami-desktop.exe`.

## Documentação e créditos

- [`docs/CREDITS.md`](docs/CREDITS.md) — modelo 3D de referência (白ー (shiro),
  por Sweet_nya555) e fontes OFL.
- [`docs/design-ref/DESIGN_REFERENCE.md`](docs/design-ref/DESIGN_REFERENCE.md) —
  referência visual e decisões de tema.
- [`docs/ui-parity/`](docs/ui-parity/) — comparações WEB/DESKTOP das telas.
- [`PACKAGE_README.md`](PACKAGE_README.md) — instruções que vão no zip.
- README da raiz — visão do workspace, infraestrutura e backend.
