# Usami

Workspace da companion virtual Usami: interface web, app desktop nativo e o
backend local que conecta chat, voz, memória e vínculo. Este README descreve o
workspace como um todo; cada componente tem documentação própria (links no fim).

## Visão geral

```
┌──────────────┐   HTTP 0.0.0.0:8787     ┌───────────────────────┐
│ usami (web)  │ ──────────────────────► │ usami/server.mjs      │
│ React + 3D   │                         │ API local do core     │
└──────────────┘                         └──────────┬────────────┘
┌──────────────┐                                    │ webhooks
│ usami-desktop│ ─────────────────────────────────► │
│ Rust + GPUI  │                         ┌──────────▼────────────┐
└──────────────┘                         │ n8n (Docker, :5678)   │
                                         │ Postgres 16           │
                                         └──────────┬────────────┘
                                                    │
                          LLM (OpenRouter / NVIDIA) │ Gemini TTS/STT
```

As duas interfaces (web e desktop) falam com a **mesma API local**; nenhuma
delas guarda chaves de provedor. O backend lê as credenciais do `.env` (ou de
variáveis de ambiente) e chama os provedores direto; o n8n recebe os eventos do
chat e executa as automações agendadas (notas no Obsidian, consolidação de
memória, relatórios).

## Componentes

| Caminho | O que é | Stack | Versionamento |
| --- | --- | --- | --- |
| `usami/` | Runtime principal da companion e API local (`/api/*` na porta 8787). Interface web com avatar 3D/Live2D, chat, TTS, STT, memória, personalidade, vínculo e uma página de transmissão (`index-live.html`) para browser source do OBS. | React 19 + Vite, Three.js + `@pixiv/three-vrm`, Node (`server.mjs`), Gemini (TTS/STT), OpenRouter/NVIDIA (LLM) | Repo próprio `UsamiVtuber/Usami-Core` (fora deste workspace git) |
| `usami-desktop/` | App nativo Windows da mesma companion: palco VRM, chat, TTS com lipsync, chamada mãos-livres, Central de Vínculo, Central do Agente e overlay do League of Legends (Usami.GG). | Rust (edition 2024) + GPUI 0.2.2, `gpui-vrm` (WGPU), `cpal`/`rodio`, `reqwest` | Repo de código `usami-workspace`; releases em `UsamiVtuber/usami-desktop` (v0.6.1) |
| `gpui-vrm/` | Motor VRM/glTF/VRMA nativo usado pelo desktop (MToon cel-shading, SpringBone, blendshapes, lipsync, look-at). | Rust + WGPU | Crate local (`path` no `Cargo.toml` do desktop); fora deste git |
| `usami-monitor/` | Painel operacional do notebook Linux que roda o Docker/n8n. | FastAPI + HTML estático (sem build) | Repo próprio `UsamiVtuber/usami-monitor` |
| `infra/n8n-local/` | Stack Docker do n8n + Postgres 16, exports de workflows e variante `nyarch/` para o notebook. | docker-compose, n8n, PostgreSQL 16, SearXNG (nyarch) | Dentro deste repo; segredos em `.env` local |
| `infra/observability/` | Langfuse local (traces do chat, spans de memória/ferramentas e gerações de LLM). Roda só no loopback; a telemetria do core é opcional e fica no-op sem as chaves. | docker-compose, Langfuse 2, PostgreSQL 16 | Dentro deste repo; segredos em `.env` local |
| `assets/` | Material de terceiros: modelos (VRM/GLB/FBX), Cubism SDK, imagens e ideias. Licenças próprias; não redistribuir sem checar. | — | Só 4 imagens versionadas; o restante fica fora do git |
| `usami/data/` | Dados locais do runtime: memória, vínculo, humor, conquistas, propostas e caches. O caminho é `data/` relativo ao diretório de onde a API roda (`usami/data/` via `npm run api`); `USAMI_DATA_DIR` sobrescreve. | — | Fora do git |
| `vault/` | Vault Obsidian privado (`Usami Obsidian`). | — | Fora do git; conteúdo privado |
| `dist/` | Artefatos de build/release gerados localmente (staging e zips). | — | Fora do git |
| `Agents/` | Agentes e skills usados no desenvolvimento do workspace. | — | Fora do git (`Agents/` no `.gitignore`) |
| `airi/`, `OpenAmika/` | Projetos open-source mantidos como referência. Não fazem parte do runtime. | — | Fora do git |
| `_archive/`, `_backup/` | Conteúdo único preservado e backups locais. | — | `_archive` versionado; `_backup` fora do git |

## Portas e serviços

| Porta | Serviço | Exposição |
| --- | --- | --- |
| `8787` | API local do core (`usami/server.mjs`) | `0.0.0.0` por padrão (loopback + rede local); `USAMI_API_HOST=127.0.0.1` restringe. `Host`/`Origin` na allowlist; rotas sensíveis exigem `x-usami-token` |
| `5173` | Vite dev server do web | Local; proxy `/api` para `127.0.0.1:8787` (injeta o token de dev). `npm run dev:lan` publica na rede |
| `5678` | Editor n8n | Stack local: `0.0.0.0:5678` (visível na LAN). No notebook (`nyarch`): `10.42.0.1` e `127.0.0.1` |
| `8000` | Usami Monitor (FastAPI) | Somente `127.0.0.1` **no notebook Linux**; acesso por túnel SSH |
| `3000` | Langfuse (UI e API de ingestão) | Somente `127.0.0.1`; sobe com `infra/observability` |
| `5432` | Postgres do compose n8n | Interno do Docker |
| `5433` | Postgres do Langfuse | Somente `127.0.0.1`, para inspeção local |

## Requisitos

- **Windows 10/11 x64** para o app desktop e o uso do dia a dia.
- **Node.js 22+** (recomendado 22 ou 24) e npm para o core web (`usami/`).
- **Docker** (Desktop ou Engine) para as stacks n8n/Postgres e Langfuse.
- **Rust** estável com suporte a **edition 2024** para compilar o desktop.
- **Python 3.10+** opcional, para o pipeline de ingestão (`usami/tools/ingest`) e o Whisper local.
- **Notebook Linux** apenas para o `usami-monitor` e para a variante `nyarch` do n8n.

## Como rodar

1. **Infraestrutura (n8n + Postgres)**
   ```powershell
   cd infra/n8n-local
   Copy-Item .env.example .env   # preencha POSTGRES_PASSWORD no arquivo
   docker compose up -d
   ```
   O editor fica em `http://localhost:5678`. Workflows e detalhes em
   `infra/n8n-local/README.md`.

2. **Core + interface web**
   ```powershell
   cd usami
   npm install
   npm run api    # API em 0.0.0.0:8787 (restrinja com USAMI_API_HOST=127.0.0.1)
   npm run dev    # Vite em paralelo
   ```
   O front web é desenvolvimento/prototipagem; a página de transmissão para OBS
   fica em `http://127.0.0.1:5173/index-live.html` (ver `usami/docs/live-streaming.md`).

3. **App desktop**
   ```powershell
   cd usami-desktop
   cargo run
   ```
   Sem backend no ar a janela abre, mas chat/voz não respondem. O avatar exige um
   `.vrm` próprio (não acompanha o repositório nem as releases).

4. **Monitor (notebook Linux)** — copie `usami-monitor/` para o notebook e siga o
   `README.md` dele; o acesso é via túnel SSH para `127.0.0.1:8000`.

## Endpoints da API local

`usami/server.mjs` expõe, entre outros:

- conversa e voz: `/api/chat`, `/api/chat/activity`, `/api/tts`, `/api/stt`
- memória e personalidade: `/api/memory`, `/api/memory/forget`, `/api/memory/maintenance`, `/api/personality`, `/api/mood`, `/api/mood/event`, `/api/achievements`
- vínculo e agente: `/api/bond`, `/api/bond/event`, `/api/agent/turn`, `/api/agent/self`, `/api/agent/skills`, `/api/agent/suggestions`, `/api/agent/briefing`, `/api/agent/proposals`, `/api/agent/presence`
- orquestração: `/api/n8n/command`, `/api/n8n/commands` (`GET`/`DELETE`), `/api/n8n/commands/ack`
- contexto, aprendizado e planejamento: `/api/rag/context`, `/api/learning`, `/api/learning/verdict`, `/api/plan`, `/api/technique(s)`
- mídia, proatividade e transmissão: `/api/media/analyze`, `/api/proactive/state`, `/api/proactive/trigger`, `/api/live/*`

Rotas de memória, personagem, agente, STT, mídia e telemetria exigem `x-usami-token`; as demais usam a allowlist de `Host`/`Origin`.

## Testes

| Componente | Comando | Cobertura |
| --- | --- | --- |
| `usami/` | `npm test` | unit, chat/API, technique, planner, ingest (Python) e sites |
| `usami-desktop/` | `cargo test` | 51 testes: contratos de chat/TTS/STT/mídia/agente, WAV do gravador, cadeado do microfone, despedida, Central do Agente, módulo LoL e resolução de assets |
| `usami-monitor/` | `python -m pytest tests -q` | guardas do painel (argv sem shell, contenção de caminho, CSRF); no Windows uma asserção de caminho falha pelo separador `\` — rode no notebook Linux |

## QA review pela Usami (CI)

Cada PR para `master` recebe um review automático da **Usami** (`.github/workflows/qa-review.yml`
+ `scripts/qa-review.mjs`): o diff vai para o `BRAIN_MODEL` (NVIDIA Nemotron 3 Ultra, via API
NVIDIA direta) e o veredito é postado como comentário na PR. O merge continua sendo do mantenedor.

- **Runner**: self-hosted no notebook Linux, labels `[self-hosted, linux, usami-notebook]`,
  `timeout-minutes: 25`. O job usa o `gh` autenticado como **Usamiin** no keychain do SO —
  nenhum token do GitHub fica no CI.
- **Provisionar o runner**: registre no nível da organização com
  `./config.sh --url https://github.com/UsamiVtuber --token <token da UI> --name luke --labels linux,usami-notebook --unattended --replace`
  (o comando que a UI gera não inclui `--name`/`--labels`, e sem `usami-notebook` nenhum job casa).
  O serviço é `actions.runner.UsamiVtuber.luke.service`, sem `Restart=always` — depois de reiniciar
  o notebook, garanta que ele volta com
  `sudo systemctl enable --now actions.runner.UsamiVtuber.luke.service`.
- **Runner group `Default`**: precisa de **"Allow public repositories"** marcado
  (Settings → Actions → Runner groups). Com essa opção desligada o runner aparece **Idle** e os
  jobs deste repositório público ficam `queued` para sempre, sem erro visível.
- **Host**: Node.js 22+ e `gh` autenticado como Usamiin (`gh auth status` roda como primeiro check
  e falha o job se o login não estiver lá).
- **Secret**: `NVIDIA_API_KEY` como secret no nível da organização (ou do repo) com acesso a este
  repositório; o workflow o expõe só ao job (`${{ secrets.NVIDIA_API_KEY }}`). O modelo vem da
  variável `BRAIN_MODEL` no próprio workflow.
- **Scan de segredos (opcional)**: antes da chamada ao modelo, o diff passa por `gitleaks`
  (`ghcr.io/gitleaks/gitleaks:v8.30.1` via `docker run`, sem instalar nada no runner). Não bloqueia:
  se acusar algo, o step emite um `::warning` com os valores redigidos e o review continua. Isso
  importa porque o texto do diff inteiro é enviado à API externa da NVIDIA.
- **Re-run manual**: `gh workflow run qa-review.yml -f pr_number=5`. Enquanto o workflow ainda não
  estiver na `master`, acrescente `--ref feat/qa-review-workspace` (ou a branch que o contém).
  Um push novo na PR cancela o review em andamento (`concurrency`).

## Releases

- **Desktop**: publicação **local**, sem PAT em CI, via `usami-desktop/scripts/release.ps1`
  (usa o `gh` autenticado de quem publica). Canal público:
  `UsamiVtuber/usami-desktop`. Última versão: **v0.5.1** (Windows x64, ~12,2 MiB,
  2026-11-11).
- O zip **não inclui** arquivos `.vrm`/`.glb`/`.fbx`: o script copia
  `usami-desktop/assets/` como está e hoje essa pasta só tem fundo, fontes e
  ícones. Confira antes de publicar.
- **Core** e **monitor** têm repositórios próprios e não são publicados por este
  workspace.

## Segurança e privacidade

- Nenhuma chave, token ou senha em código, config versionada ou logs. Segredos
  ficam em arquivos `.env` locais (n8n e core) ou em variáveis de ambiente.
- A API escuta em `0.0.0.0:8787` por padrão. A borda é a allowlist de `Host` e
  `Origin` mais o `x-usami-token` nas rotas sensíveis; para voltar ao loopback,
  defina `USAMI_API_HOST=127.0.0.1`.
- A stack n8n local publica `0.0.0.0:5678` (visível na LAN); a variante `nyarch`
  publica só `10.42.0.1` e `127.0.0.1`. O monitor e o Langfuse escutam apenas em
  `127.0.0.1`; o monitor é acessado por túnel SSH e não tem autenticação própria.
- O `vault/` é privado: não indexar, copiar ou publicar conteúdo.
- O app desktop não guarda chaves de provedor; ele fala com a API local e, se o
  módulo LoL estiver ligado, lê a Live Client Data API do jogo e baixa o
  DataDragon (desligável com `USAMI_LOL_ENABLED=false`).

## Documentação por componente

- Core: `usami/README.md`, `usami/AGENTS.md` e `usami/docs/architecture.md`
  (mapa da USAMI 2.0 em `usami/docs/usami-2.0-architecture-map.md`)
- Desktop: `usami-desktop/README.md`, `usami-desktop/docs/KNOWN_ERRORS.md`,
  `usami-desktop/docs/design-ref/DESIGN_REFERENCE.md`,
  `usami-desktop/docs/ui-parity/` e `usami-desktop/docs/CREDITS.md`
- Infra: `infra/n8n-local/README.md`, `infra/n8n-local/workflows/README.md` e
  `infra/observability/README.md`
- Monitor: `usami-monitor/README.md`
- Transmissão: `usami/docs/live-streaming.md`
- Plano de produto em andamento: `KYNDRED_INSPIRED_PLAN.md`

## Estado atual (2026-11-15)

- Desktop **v0.5.1** publicado: Central de Vínculo, Central do Agente, painel de
  conversa em formato de celular, overlay Usami.GG para League of Legends, STT
  parcial na legenda e rotas protegidas por token.
- Core com turno agêntico (classificador determinístico + ferramentas), memória
  consolidada, proatividade, bridge para code agents e página de transmissão
  para OBS (`index-live.html`).
- Monitor com suíte de testes (pytest) e variante `nyarch` operando no notebook
  Linux, atrás de túnel SSH.
