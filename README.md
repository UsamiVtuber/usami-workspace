# Usami

Workspace da companion virtual Usami: interface web, app desktop nativo e o
backend local que conecta chat, voz, memória e vínculo. Este README descreve o
workspace como um todo; cada componente tem documentação própria (links no fim).

## Visão geral

```
┌──────────────┐   HTTP 127.0.0.1:8787   ┌───────────────────────┐
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
delas guarda chaves de provedor. O backend lê as credenciais do `.env` e usa o
n8n como orquestrador dos fluxos de conversa, memória e voz.

## Componentes

| Caminho | O que é | Stack | Versionamento |
| --- | --- | --- | --- |
| `usami/` | Runtime principal da companion e API local (`/api/*` na porta 8787). Interface web com avatar 3D/Live2D, chat, TTS, STT, memória, personalidade e vínculo. | React 19 + Vite, Three.js + `@pixiv/three-vrm`, Node (`server.mjs`), Gemini (TTS/STT), OpenRouter/NVIDIA (LLM) | Repo próprio `UsamiVtuber/Usami-Core` (fora deste workspace git) |
| `usami-desktop/` | App nativo Windows da mesma companion: palco VRM, chat, TTS com lipsync e chamada mãos-livres. | Rust (edition 2024) + GPUI 0.2.2, `gpui-vrm` (WGPU), `cpal`/`rodio`, `reqwest` | Repo de código `usami-workspace`; releases em `UsamiVtuber/usami-desktop` (v0.2.0) |
| `gpui-vrm/` | Motor VRM/glTF/VRMA nativo usado pelo desktop (MToon cel-shading, SpringBone, blendshapes, lipsync, look-at). | Rust + WGPU | Crate local (`path` no `Cargo.toml` do desktop); fora deste git |
| `usami-monitor/` | Painel operacional do notebook Linux que roda o Docker/n8n. | FastAPI + HTML estático (sem build) | Repo próprio `UsamiVtuber/usami-monitor` |
| `infra/n8n-local/` | Stack Docker do n8n + Postgres 16, exports de workflows e variante `nyarch/` para o notebook. | docker-compose, n8n, PostgreSQL 16, SearXNG (nyarch) | Dentro deste repo; segredos em `.env` local |
| `assets/` | Material de terceiros: modelos (VRM/GLB/FBX), Cubism SDK, imagens e ideias. Licenças próprias; não redistribuir sem checar. | — | Só 4 imagens versionadas; o restante fica fora do git |
| `data/` | Dados locais do runtime (ex.: `memory.json`). | — | Fora do git |
| `vault/` | Vault Obsidian privado (`Usami Obsidian`). | — | Fora do git; conteúdo privado |
| `dist/` | Artefatos de build/release gerados localmente (staging e zips). | — | Fora do git |
| `Agents/` | Agentes e skills usados no desenvolvimento do workspace. | — | Fora do git (`Agents/` no `.gitignore`) |
| `airi/`, `OpenAmika/` | Projetos open-source mantidos como referência. Não fazem parte do runtime. | — | Fora do git |
| `_archive/`, `_backup/` | Conteúdo único preservado e backups locais. | — | `_archive` versionado; `_backup` fora do git |

## Portas e serviços

| Porta | Serviço | Exposição |
| --- | --- | --- |
| `8787` | API local do core (`usami/server.mjs`) | Somente `127.0.0.1` |
| `5173` | Vite dev server do web | Local; proxy `/api` para `127.0.0.1:8787` (injeta o token de dev) |
| `5678` | Editor n8n | `127.0.0.1`; no notebook (`nyarch`) também `10.42.0.1` da rede do cabo |
| `8000` | Usami Monitor (FastAPI) | Somente `127.0.0.1` **no notebook Linux**; acesso por túnel SSH |
| `5432` | Postgres do compose | Interno do Docker |

## Requisitos

- **Windows 10/11 x64** para o app desktop e o uso do dia a dia.
- **Node.js + npm** para o core web (`usami/`).
- **Docker** (Desktop ou Engine) para a stack n8n/Postgres.
- **Rust** com suporte a **edition 2024** para compilar o desktop.
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
   npm run api    # API local em 127.0.0.1:8787
   npm run dev    # Vite em paralelo
   ```

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

- conversa e voz: `/api/chat`, `/api/tts`, `/api/stt`
- memória e personalidade: `/api/memory`, `/api/memory/forget`, `/api/personality`
- orquestração: `/api/n8n/command`, `/api/n8n/commands`, `/api/n8n/commands/ack`
- contexto e aprendizado: `/api/rag/context`, `/api/learning`, `/api/learning/verdict`, `/api/plan`, `/api/technique(s)`
- mídia e proatividade: `/api/media/analyze`, `/api/proactive/state`, `/api/proactive/trigger`

## Testes

| Componente | Comando | Cobertura |
| --- | --- | --- |
| `usami/` | `npm test` | unit, chat/API, technique, planner, ingest (Python) e sites |
| `usami-desktop/` | `cargo test` | 19 testes: contratos de chat/TTS/STT, WAV do gravador, cadeado do microfone, resolução de assets |
| `usami-monitor/` | — | sem suíte automatizada; uso restrito a localhost |

## Releases

- **Desktop**: publicação **local**, sem PAT em CI, via `usami-desktop/scripts/release.ps1`
  (usa o `gh` autenticado de quem publica). Canal público:
  `UsamiVtuber/usami-desktop`. Última versão: **v0.2.0** (Windows x64, ~11,9 MiB).
- O zip **não inclui** arquivos `.vrm`/`.glb`/`.fbx`; modelos são distribuídos à parte.
- **Core** e **monitor** têm repositórios próprios e não são publicados por este
  workspace.

## Segurança e privacidade

- Nenhuma chave, token ou senha em código, config versionada ou logs. Segredos
  ficam em arquivos `.env` locais (n8n e core).
- API, n8n e monitor escutam apenas em `127.0.0.1`; o monitor é acessado por túnel SSH.
- O `vault/` é privado: não indexar, copiar ou publicar conteúdo.
- O app desktop não guarda chaves; ele só fala com a API local.

## Documentação por componente

- Core: `usami/AGENTS.md` e `usami/docs/architecture.md`
- Desktop: `usami-desktop/README.md`, `usami-desktop/docs/design-ref/DESIGN_REFERENCE.md`,
  `usami-desktop/docs/ui-parity/` e `usami-desktop/docs/CREDITS.md`
- Infra: `infra/n8n-local/README.md` e `infra/n8n-local/workflows/README.md`
- Monitor: `usami-monitor/README.md`
- Plano de produto em andamento: `KYNDRED_INSPIRED_PLAN.md`

## Estado atual (2026-11-08)

- Desktop **v0.2.0** publicado, com tema escuro, fontes OFL embutidas e painel de
  configurações com as seis abas do web (controles que dependem do web aparecem
  desabilitados).
- Core web segue como referência de comportamento; memória e personalidade são
  editadas por lá.
- Monitor e variante `nyarch` operam no notebook Linux, atrás de túnel SSH.
