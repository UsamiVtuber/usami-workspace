# Usami Desktop

Aplicativo nativo da companion Usami para Windows, escrito em **Rust** sobre o
**GPUI 0.2.2** (framework de UI da Zed Industries). Abre uma janela com o palco
VRM 3D, chat com o backend local, TTS com lipsync e um modo chamada mãos-livres
com microfone, sem navegador embutido. Em volta do palco ficam o painel de
conversa em formato de celular, a Central de Vínculo em estilo visual novel, a
Central do Agente e um overlay para o League of Legends (Usami.GG).

- **Versão atual:** 0.6.1 (`Cargo.toml`, seção Sistema do Ajustes).
- **Canal de releases:** [`UsamiVtuber/usami-desktop`](https://github.com/UsamiVtuber/usami-desktop) (público).
- **Distribuição:** zip Windows x64 com o executável e a pasta `assets/`. O
  pacote **não** inclui modelos `.vrm` (ver [Modelo e assets](#modelo-e-assets)).

## O que o app faz

- Renderiza o avatar VRM em um palco interativo (arrastar orbita; a rolagem
  aproxima e afasta).
- Ícone nativo e reações de humor: o executável embute o ícone oficial (olhos `><`),
  e durante a execução o ícone da janela, da barra de tarefas e da bandeja muda
  dinamicamente conforme o humor e as reações da Usami em tempo real (`assets/icons/emotions/`).
- Conversa com a API local (`/api/chat`) e exibe as mensagens em balão e no
  painel de conversa em formato de celular.
- Mostra o que ela está fazendo durante o turno (`/api/chat/activity`):
  pensando, executando uma ferramenta (com o nome) ou respondendo.
- Toca a resposta com a voz configurada no backend (`/api/tts`), com streaming
  PCM para começar a falar antes do áudio terminar e lipsync pelo volume.
- Em chamada, ouve o microfone em contínuo, segmenta a fala por silêncio e
  transcreve (`/api/stt`) sem clique; enquanto você fala, trechos parciais
  alimentam a legenda ao vivo no topo.
- Oferece a página de Ajustes (voz, personalidade, memória, aparência,
  integrações, sistema e uma seção Avançado) e, no celular, atalhos para
  proatividade, mudo, histórico e vínculo. Memória e personalidade só são
  editáveis com `USAMI_API_TOKEN` configurado no app (e no core).
- Abre a Central do Agente (`/api/agent/*`): skills, execuções, permissões,
  confirmações e sugestões, com badge de pendências no topo.
- Vira um overlay transparente always-on-top (Usami.GG) quando há partida de
  League of Legends: lê a Live Client Data API local (`https://127.0.0.1:2999`),
  monta as reações e fala pelo mesmo `/api/chat` e `/api/tts`.
- Vive na bandeja com sempre-no-topo e iniciar-com-o-Windows; `Ctrl+Alt+U`
  mostra/esconde a janela e `Ctrl+Alt+L` alterna o clique no overlay LoL.
- Tem modo companhia (ícone da lua): emoção `sleepy` e estado interno
  `MODO COMPANION` por até 30 minutos.
- Lembra a conversa entre sessões: as últimas 100 mensagens, o mudo, a
  geometria da janela, os anexos e o estado da proatividade ficam em
  `%APPDATA%\Usami`.
- Sente a ausência de imediato: ao minimizar, esconder na bandeja ou perder o
  foco o app entra no estado interno `AUSENTE` (usado nos eventos proativos);
  ao voltar ela cumprimenta de acordo com o tempo fora, e detecta o fechamento
  abrupto da sessão anterior. Com a janela em primeiro plano e sem interação,
  ela puxa assunto aos ~3,5, 7 e 10 minutos.
- Segura o fechamento para se despedir: o X, o Alt+F4 e o "Sair" da bandeja
  pedem a fala de despedida e a janela só fecha quando o balão aparece e o
  áudio termina (no mudo, ~1,2 s), com teto de 10 s. Fechar de novo durante a
  despedida sai na hora.
- Mostra o vínculo da VN a partir do core local (`GET /api/bond`): nível e EXP
  no pill do topo, medidores de carinho/humor/companhia, resumo e histórico de
  momentos na Central de Vínculo. Cada mensagem enviada rende XP pelas mesmas
  regras do web (`POST /api/bond/event`, com `spoken` em chamada) e encerrar uma
  chamada rende companhia por minuto inteiro. São cinco níveis, até "Almas
  Gêmeas".

### Persistência local

| Arquivo | Conteúdo |
| --- | --- |
| `conversation.json` | últimas 100 mensagens (texto, emoção e metadados de anexo). |
| `ui.json` | mudo e posição/tamanho da janela (descartada se a geometria for inválida). |
| `session.json` | início, batimento a cada 60 s e `clean_shutdown` — é o que permite notar um kill abrupto na próxima abertura. |
| `proactivity_config.json` / `proactivity_seen.json` | interruptor da proatividade e ids de eventos já entregues. |
| `media/` | imagens e documentos anexados ao chat (os bytes não entram no JSON da conversa). |
| `lol/ddragon/` | cache por patch dos dados estáticos do LoL (campeões, itens, runas). |

As gravações usam arquivo temporário + rename para não deixar JSON truncado se
o processo morrer no meio. O vínculo (medidores de afeto) não mora aqui: ele é
compartilhado por dispositivo e pertence ao core local (`usami/data/bond.json`),
lido e atualizado via `/api/bond` e `/api/bond/event`. Humor e conquistas também
são do core (`/api/mood`, `/api/achievements`).

### Por que nativo

O app renderiza com a GPU pela stack do GPUI/WGPU, toca PCM direto em `rodio`
(sem camada de mídia do navegador; o `cpal` fica só na captura do microfone) e
mantém a UI em Rust. Isso evita embarcar um Chromium e dá controle do áudio por
amostra, necessário para o streaming de TTS, o lipsync e a VAD da chamada.

## Requisitos

- **Windows 10/11 x64.** O pacote publicado é só para essa plataforma.
- **Rust** estável recente com suporte a `edition 2024` (o código usa
  let-chains).
- **Backend da Usami** em `http://127.0.0.1:8787` — `cd usami; npm run api`.
  O core escuta em `0.0.0.0:8787` por padrão (`USAMI_API_HOST`); o app usa
  `127.0.0.1:8787` a menos que `USAMI_API_BASE` aponte outro host. Sem o
  backend a janela abre, mas chat, TTS e STT não respondem.
- **`USAMI_API_TOKEN`** no próprio app e no core para as rotas protegidas
  (memória, personalidade, STT, mídia e Central do Agente). Sem o token o
  restante do app funciona; essas telas mostram o aviso e ficam bloqueadas.
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

O token das rotas protegidas vem de `USAMI_API_TOKEN` no ambiente; sem ele, o
app procura `USAMI_API_TOKEN=` em um `.env` no diretório atual, no diretório pai
e ao lado do `.exe`.

## Arquitetura

| Caminho | Responsabilidade |
| --- | --- |
| `src/main.rs` | Janela e estado do app, palco 3D, balão, compositor e orquestração do fluxo de conversa/chamada, incluindo as camadas de UI. |
| `src/views/` | Camadas de render: palco, HUD, controles, celular, Central de Vínculo, Ajustes, Central do Agente e overlay LoL. |
| `src/model.rs`, `src/input_editor.rs`, `src/text_input.rs` | Modelos de mensagem e abas, edição do compositor e tokenização do texto. |
| `src/theme.rs` | Tokens de tema (paleta e tipografia), fontes embutidas e `glow`. |
| `src/api.rs` | Cliente HTTP (`reqwest`): `/api/chat` e atividade, TTS, STT, memória, vínculo, humor, personalidade, conquistas, proatividade, mídia e agente; timeouts e parsing do streaming de TTS. |
| `src/audio.rs` | Saída de áudio com `rodio`; fila de PCM contínua, sem pausas entre trechos, e envelope de fala para o lipsync. |
| `src/recorder.rs` | Captura do microfone em thread própria (`cpal`), VAD por frames de 20 ms, `MicGate` e codificação WAV PCM 16-bit mono. |
| `src/voice.rs` | Chamada e escuta: microfone, mudo, portão do mic, modo companhia, status e entrega do turno. |
| `src/lifecycle.rs`, `src/presence.rs` | Ausência/retorno, despedida, provocações de ociosidade e presença no Windows (bandeja, atalhos, topmost e autostart). |
| `src/storage.rs` | Persistência local em `%APPDATA%\Usami`: conversa, preferências, sessão, proatividade e anexos. |
| `src/bond.rs`, `src/proactive.rs`, `src/memory_personality.rs` | Eventos de vínculo, fila de eventos proativos e leitura/gravação de memória e personalidade. |
| `src/agent_center_model.rs`, `src/views/agent_center.rs` | Central do Agente: agrupamento/formatação pura e abas de skills, execuções, permissões, confirmações e sugestões. |
| `src/lol/` | Usami.GG: Live Client Data API, DataDragon com cache, fila de eventos, prompt do companion e overlay Win32. |
| `src/media.rs` | Mídia anexada ao chat (preview e envio em base64). |
| `build.rs` | Compilação de recursos PE do Windows e embutimento do ícone nativo (`winres`). |
| `../gpui-vrm` | Crate local do motor VRM/glTF/VRMA (WGPU, MToon cel-shading, SpringBone, blendshapes, lipsync e look-at IK). |
| `PACKAGE_README.md` | README que acompanha o zip de release (instalação, requisitos, aviso do SmartScreen). |
| `scripts/release.ps1` | Build e publicação local da release no repo público. |
| `docs/KNOWN_ERRORS.md` | Registro de erros e correções já vistas no componente. |

Principais dependências: `gpui` 0.2.2, `cpal` 0.18.2, `rodio` 0.23.0,
`reqwest` 0.13.5 (`json`, `multipart`, `stream`), `tokio` 1.54.1, `serde`
1.0.229, `serde_json` 1.0.154 e `thiserror` 2.0; no Windows, `windows-sys` 0.61
(bandeja, atalhos, registro e janela), `raw-window-handle` 0.6 e `winres` 0.1
(build-dependency para recursos nativos).

## Interface

- **Palco:** avatar VRM sobre a imagem de fundo resolvida em
  `USAMI_BACKGROUND`/`assets/background.png`; arrastar orbita e a rolagem
  aproxima/afasta (interação do `gpui-vrm`).
- **Header superior:** botões circulares de vínculo (`Lv<n>`), Central do
  Agente, celular e Ajustes, além do crédito do modelo. Ele desaparece quando
  o app fica ocioso e o avatar entra em modo retrato.
- **Balão de fala:** mostra a última resposta da Usami, com destaque tipográfico.
- **Celular da Usami:** o botão de chat abre um painel em formato de smartphone,
  com balões no estilo WhatsApp (ticks de lido, selo de humor e badge de skill),
  prévia da mídia anexada, alternância microfone/enviar e uma tela de Ajustes
  rápida (proatividade, mudo, limpar histórico, vínculo e sobre).
- **Central de Vínculo (visual novel):** o pill `Lv<n>` abre o overlay com os
  capítulos 01 Início, 02 Diário, 03 Álbum de Memórias, 04 Laços,
  05 Conquistas, 06 Humor, 07 Perfil da Usami e 08 Ajustes.
- **Central do Agente:** o botão de skills abre o painel lateral com Skills,
  Execuções, Permissões, Confirmações e Sugestões; pendências aparecem como
  badge no botão.
- **Compositor e dock:** campo de texto com anexo de imagem, telefone (iniciar
  chamada), volume (mudo), lua (modo companhia) e enviar. Em chamada, o dock
  mostra escuta (pulse), celular, encerrar, mudo e companhia.
- **Legenda ao vivo:** durante a escuta, trechos parciais vão ao `/api/stt` a
  cada ~1,2 s e o texto já entendido aparece no topo.
- **Overlay LoL:** com partida em andamento o app vira o overlay Usami.GG,
  transparente e sempre no topo; `Ctrl+Alt+L` alterna o clique através dele.
- **Modo mascote (Desktop Mate):** o botão de minimizar, a bandeja ou
  `Ctrl+Alt+M` encolhem o app para o avatar sem moldura, sempre no topo e fora
  da barra de tarefas; duplo clique no avatar volta ao app. No modo mascote, o
  clique direito abre o menu (falar por voz, seguir o cursor, configurações e
  voltar), o compositor envia texto pelos mesmos endpoints do chat e o
  microfone reaproveita o STT do chamado. A preferência `mascot_follow`
  (Ajustes → Sistema) faz a cabeça e os olhos acompanharem o ponteiro pela tela
  inteira; sem ela, só o hover dentro da janela move o olhar.

### Painel de configurações

Há duas superfícies. A engrenagem do header abre o celular na tela de Ajustes
rápida (proatividade, microfone, conversas, vínculo e sobre). A página completa
fica no capítulo 08 (Ajustes) da Central de Vínculo, com rolagem vertical e
categorias:

| Categoria | Conteúdo | Estado no desktop |
| --- | --- | --- |
| Voz e áudio | voz oficial, tags de som (`<laugh>`, `<chuckle>`, `<sigh>`, `<short pause>`), STT e privacidade da chave. | leitura |
| Personalidade e comportamento | traços, instruções extras, salvar e restaurar. | edição (exige `USAMI_API_TOKEN`) |
| Memória e privacidade | fatos por seção, medidores de Laços e exclusão. | edição (exige token) |
| Aparência e interface | renderizador, modelo e cenário. | leitura |
| Integrações | URL do backend, estado do token e proatividade. | leitura + interruptor |
| Sistema | versão, tecnologias, créditos e privacidade. | leitura |
| Avançado | telemetria do último turno e limpeza do histórico local. | ação local |

Sem token, as seções de memória e personalidade mostram o aviso de
`USAMI_API_TOKEN` e ficam bloqueadas. A exclusão de memória não pede digitar a
palavra: é preciso liberar as exclusões e o app envia `confirm: "APAGAR"` na
requisição. O rig e o cenário completo continuam no app web; dados sensíveis e
credenciais seguem no servidor local.

## Tema e tipografia

O tema escuro segue a direção de arte "Midnight Romance" definida em
`src/theme.rs`: fundo `#100819`, superfície `#20102F`, rosa de destaque
`#F052B5`, rosa claro `#F5C3E1`, lavanda `#A77ABF` e texto `#F6EAF3`, com
espaçamento e raios compartilhados pela UI. As fontes são **embutidas** e
carregadas de `assets/fonts/`, sem download em tempo de execução:

- **DM Sans** — texto de interface.
- **Cormorant Garamond** — títulos editoriais, fala da personagem e badge de
  vínculo (incluindo os cortes itálico e semibold).
- **Noto Sans JP** — glifos japoneses (subset embutido).

Todas estão sob a SIL Open Font License 1.1. Autores, arquivos e textos das
licenças em [`docs/CREDITS.md`](docs/CREDITS.md).

## Modelo e assets

O app resolve assets ao lado do executável: se existir uma pasta `assets/` na
mesma pasta do `.exe` (caso do pacote), ela é a base; caso contrário usa a pasta
da crate (desenvolvimento). O arquivo `.vrm` **não** é distribuído — por
licença, o usuário aponta um modelo próprio.

A variável de ambiente, quando definida e existente, vence; sem ela o app tenta
os candidatos na ordem abaixo, do primeiro para o último existente:

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
| `USAMI_API_TOKEN` | — | Token enviado como `x-usami-token` às rotas protegidas; sem ele o app procura em um `.env` no diretório atual, no pai e ao lado do `.exe`. O valor nunca entra no executável. |
| `USAMI_LOL_ENABLED` | ligado | `0`, `false` ou `no` desliga o módulo Usami.GG (sem polling da porta 2999 e sem baixar o DataDragon). |
| `USAMI_VRM_PATH` | ver tabela acima | Arquivo `.vrm` do avatar. |
| `USAMI_VRMA_DIR` | ver tabela acima | Pasta de clipes `.vrma`. |
| `USAMI_BACKGROUND` | `assets/background.png` | Imagem de fundo da janela. |

O cliente HTTP usa **5 s** para conectar e **90 s** para a requisição completa;
o TTS em streaming sobe esse teto para **180 s** para não cortar falas longas.

## API local

O app não fala com provedores de LLM/TTS diretamente e nenhuma chave de
provedor fica no executável. Fora o core local, o único acesso de rede por
padrão é o DataDragon do módulo LoL (`ddragon.leagueoflegends.com`, com cache
local) e a leitura local da porta `2999` do cliente do jogo.

| Endpoint | Uso | Contrato |
| --- | --- | --- |
| `POST /api/chat` | enviar a mensagem e receber a resposta | JSON com `message`, `capabilities` e `sleepCompanionActive`; resposta com `reply`, `gesture`, `actions`, `speechTags`, `timing`, `affinity`, `stage`, `mood` e `agentTurn` |
| `GET /api/chat/activity` | atividade ao vivo do turno | fases `thinking`, `tool` (com o nome da ferramenta) e `responding` |
| `POST /api/tts` | sintetizar a resposta | JSON `{text, style_id, stream:true}`; com `X-TTS-Stream: 1` a resposta é PCM Int16 LE cru (sample rate no `content-type`, ex. `audio/l16; rate=24000`), senão vem o áudio completo |
| `POST /api/stt` | transcrever a fala da chamada | multipart, campo `audio` (WAV); resposta `{"text": "..."}`; exige token |
| `POST /api/media/analyze` | analisar imagem/documento anexado | JSON `{mime_type, data, filename, prompt, capabilities}`; resposta com `reply`, `actions`, `ocr`, `transcript` e `crossReference`; exige token |
| `GET /api/memory` / `POST /api/memory/forget` | ler fatos e excluir | leitura por seção; exclusão com `{kind, confirm:"APAGAR", section?, fact?, date?}`; exige token |
| `GET /api/personality` / `POST /api/personality` | ler e salvar o jeito | `{traits, customPrompt, reset}`; exige token |
| `GET /api/bond` | ler o vínculo do core | JSON `{ok, bond}` com `intimacy`, `meters` (xp, nível e progresso por medidor), `log`, `updatedAt` e `summary` |
| `POST /api/bond/event` | render XP por resposta ou chamada | JSON `{content, spoken}` ou `{kind:"call", minutes}`; devolve o vínculo atualizado |
| `GET /api/mood` / `POST /api/mood/event` | ler e registrar o humor | `{emotion, reason}` |
| `GET /api/achievements` | conquistas avaliadas pelo core | lista de conquistas com estado |
| `GET /api/proactive/events` | buscar eventos proativos | header `x-usami-activity` com `active`, `idle` ou `away`; em `away`, conversa social é confirmada como `skipped` e lembrete fica pendente |
| `POST /api/proactive/events/ack` | confirmar entrega | JSON `{acks:[{id,outcome}]}` |
| `/api/agent/*` | Central do Agente | skills, `executions?limit=`, `self?focus=tools`, `permissions`, `confirmations`, `skills/proposals`, `plans?limit=`, `suggestions?evaluate=1&limit=`, `briefing`; exige token |

O backend precisa expor esses endpoints (ver `usami/` e o README da raiz).
Erros de rede chegam traduzidos em pt-BR (servidor indisponível, token recusado,
origem não autorizada). Sem microfone, o app avisa e continua funcionando pelo
campo de texto.

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

Enquanto você fala, a cada ~1,2 s (com no mínimo 600 ms de fala) uma cópia
parcial do trecho vai ao `/api/stt` só para a legenda ao vivo do topo; o turno
final não depende dela. A transcrição exige `USAMI_API_TOKEN` no app.

O `MicGate` fecha a captura enquanto a Usami pensa ou fala (a própria voz não
vira transcrição), ao mutar e quando a escuta está desligada. Encerrar a
chamada desliga escuta, áudio e microfone.

### Detecção de PTT (Push-to-Talk) — requisito de thread

No modo *gamer* (overlay LoL ativo + PTT requerido), a tecla de PTT (por
padrão **V**, **Mouse X1** ou **Mouse X2**) é monitorada por uma task assíncrona
no GPUI.

**A task DEVE rodar no executor de foreground do GPUI (`cx.spawn`), não no
`background_executor().spawn()`.**

- `cx.spawn` agenda a closure no *event loop da UI thread* — a mesma thread
  que possui a fila de mensagens da janela (`HWND`).
- `GetAsyncKeyState` lê o bit `0x8000` ("pressionado agora"), que é **estado
  global** no Windows; porém, o bit baixo ("pressionado desde a última
  chamada") é por-thread. Para comportamento correto e previsível em todos os
  cenários (incluindo hooks de teclado globais e futuras migrações para
  `SetWindowsHookEx(WH_KEYBOARD_LL)`), a verificação precisa acontecer na UI
  thread.
- No `main.rs`, o padrão estabelecido é:
  ```rust
  cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
      let cx = cx.clone();
      async move {
          loop {
              cx.background_executor().timer(Duration::from_millis(50)).await;
              if stop.load(Ordering::Relaxed) { break; }
              // ... GetAsyncKeyState aqui roda na UI thread
          }
      }
  }).detach();
  ```
  O *sono* de 50 ms usa o background executor (thread pool), mas a checagem
  das teclas roda na UI thread logo após o `await`.

## Build e testes

```powershell
cd usami-desktop
cargo build --release --locked   # build de release para o pacote
cargo test                       # 51 testes
cargo clippy                     # sem warnings novos
```

A suíte cobre os contratos de `/api/chat`, `/api/tts` (streaming e fallback),
`/api/stt`, `/api/bond`, `/api/mood`, `/api/personality`, `/api/media/analyze` e
`/api/agent/*` contra um servidor HTTP local, erros HTTP, normalização da base,
codificação WAV, VAD e o cadeado do microfone, a despedida no fechamento, a
Central do Agente, o módulo LoL (brain, DataDragon, poller e diff de estado), a
tokenização do compositor e a resolução de caminhos de assets e ícones.

## Release

A publicação é **local e manual**, sem PAT ou secret em CI: o script usa o `gh`
já autenticado na máquina de quem publica.

```powershell
cd usami-desktop
gh auth login                          # uma vez, se ainda não estiver logado
./scripts/release.ps1 -Version 0.5.2   # próxima versão: build + zip + release
./scripts/release.ps1 -Version 0.5.2 -DryRun   # só monta o pacote, não publica
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

O script copia a pasta `assets/` inteira, então ele não filtra modelos: o que
estiver em `usami-desktop/assets/` vai para o zip. Hoje a pasta tem apenas
`background.png`, `fonts/` e `icons/` — nenhum `.vrm`, `.glb` ou `.fbx` — e o
pacote v0.5.1 publicado confirma isso. Confira a pasta antes de publicar. A
release **v0.5.1** tem 12.840.037 bytes (~12,2 MiB) e foi publicada em
2026-11-11.

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
- Memória, personalidade, STT, mídia e Central do Agente exigem
  `USAMI_API_TOKEN` definido no app e no core.
- Aparência/rig e cenário completo continuam no app web; no desktop essa seção
  é só leitura.
- O modo LoL depende do cliente do jogo na máquina (Live Client Data API em
  `https://127.0.0.1:2999`) e baixa os dados estáticos do DataDragon no primeiro
  uso de cada patch; sem rede ele usa o cache local ou o conhecimento embutido.
- O pacote exige que `assets/` fique ao lado do `usami-desktop.exe`.

## Documentação e créditos

- [`docs/CREDITS.md`](docs/CREDITS.md) — modelo 3D de referência (白ー (shiro),
  por Sweet_nya555) e fontes OFL.
- [`docs/KNOWN_ERRORS.md`](docs/KNOWN_ERRORS.md) — erros, falhas e correções já
  registrados no componente (ler antes de mexer no código).
- [`docs/design-ref/DESIGN_REFERENCE.md`](docs/design-ref/DESIGN_REFERENCE.md) —
  referência visual e decisões de tema.
- [`docs/ui-parity/`](docs/ui-parity/) — comparações WEB/DESKTOP das telas.
- [`PACKAGE_README.md`](PACKAGE_README.md) — instruções que vão no zip.
- README da raiz — visão do workspace, infraestrutura e backend.
