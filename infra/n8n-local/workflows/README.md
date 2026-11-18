# Workflows

Exports JSON dos workflows do n8n local (controle de versão). Cada arquivo é o
export de um workflow; importar no editor do n8n (Workflows → Importar) para usar.
Nada aqui é publicado/ativado automaticamente. Os exports atuais vêm com
`active: true` — exceto o `Usami - Google Assistant.json`, que vem `false` — (o
estado da instância quando foram gerados), mas importar não ativa nada: publicar
é passo manual (ou pelo MCP).

## Segredos em workflows

Nunca cole token, senha ou URL de webhook no JSON do export. O webhook do
Discord do workflow de vagas ficou inline e foi para `origin/master` de um
repositório público (commit `317c62e`, 2026-11-16); em 2026-11-17 ele foi
trocado por `{{ $env.DISCORD_VAGAS_WEBHOOK_URL }}` e a URL antiga deve ser
tratada como **comprometida**: regenere o webhook no Discord e nunca reative a
antiga. Reescrever o histórico não apaga o valor — a contenção é a rotação.

Nós que precisam de segredo leem `{{ $env.NOME }}` (a instância do Nyarch roda
com `N8N_BLOCK_ENV_ACCESS_IN_NODE: "false"`); o valor vive só em `~/n8n/.env` e
o `nyarch/docker-compose.yml` o repassa como variável do container.

## Instância autoritativa e URL real

A instância que vale é a do **Nyarch** (`http://10.42.0.1:5678`; é para onde o
MCP aponta e é o `WEBHOOK_URL` do `nyarch/docker-compose.yml`). A stack local
deste diretório (`docker-compose.yml`, publicada em `0.0.0.0:5678`) está
**desligada** e não recebe nada hoje. Consequências práticas:

- O `usami/server.mjs` resolve `N8N_WEBHOOK_URL` do environment ou do
  `A:/Usami Project/.env` (nada popula `process.env` a partir desse arquivo).
  Sem essa linha no `.env`, o default `http://localhost:5678/...` vence e o
  evento `usami.response` é enviado para lugar nenhum em silêncio.
- Export versionado ≠ publicado. Confirme na instância (`search_workflows`) antes
  de confiar num workflow. Estado atual (7 workflows, todos ativos na instância):
  `Usami - Receber Respostas` (`bJJ17mDS2I5r4De9`), `Usami - Obsidian Inbox`
  (`NnWm0PoOh4IBjtQS`), `Usami - Revisao de Tecnicas (agendado)`
  (`9GIG4GClRKAdF51j`), `Usami - Consolidacao Noturna (agendado)`
  (`qSwMxxa1bBGFkN80`), `Usami - Daily Briefing (agendado)` (`tHNaNVu11bUsNAzN`),
  `Usami - Relatorio Financeiro (agendado)` (`UpIYAszmGatDaBsW`) e
  `Usami - Google Assistant` (`2Boo8f2pnVgL5GRi`, que no export ainda vem
  `active: false`).
- A publicação é feita por **código SDK** (`validate_workflow` →
  `create_workflow_from_code`), não pelo export JSON. O export de
  `Usami - Receber Respostas` foi regerado a partir do que a instância devolveu,
  então os `id`/`webhookId`/UUIDs de nó dele são os reais; importar aquele JSON
  criaria uma segunda cópia com outros ids.

## "Revisao de Tecnicas" na instância do Nyarch

O bloqueio antigo (nó chamando `127.0.0.1:8787`, que dentro do container é ele
mesmo) está resolvido, e desde 2026-11-17 o endpoint é o **bridge**: o export e
a instância ativa (`9GIG4GClRKAdF51j`) chamam
`GET http://10.42.0.1:8787/api/techniques`, que o `usami-api-bridge.service`
(systemd de usuário no Nyarch) repassa para a API da Usami no Windows
(`10.42.0.91:8787`) pela interface física `enp2s0`.

Chamar o IP do Windows direto (`.91`) falha de dentro do container do n8n: as
duas execuções agendadas de 2026-11-17 às 04:00 deram `ECONNREFUSED` em ~11 ms
(reproduzido ao vivo no mesmo dia, exec 16254), enquanto a API respondia
normalmente no mesmo minuto (telemetria do `usami-desktop`) e o mesmo endpoint
via bridge servia dados (Spotify, exec 10694). O compose do Nyarch não tem
`extra_hosts` nem `network_mode: host`.

Com a API parada, o workflow falha todo dia às 04:00 com "API da Usami
indisponivel" conforme o `onError` de cada nó. O horário coincide com o da
`Consolidacao Noturna` (04:00). Por isso a regra para qualquer nó HTTP novo é
**de dentro do n8n, usar sempre `http://10.42.0.1:8787`**.

Regra de ouro: **o n8n não move o avatar diretamente.** Estes workflows só fazem
tarefas assíncronas/agendadas de casa (receber evento, listar/consolidar, escrever
nota) e, no máximo, **enfileiram** um comando para o cliente aplicar; nenhum
controla osso/frame-loop. Dois deles (`Daily Briefing` e `Relatorio Financeiro`)
têm nós HTTP em `POST /api/n8n/command` com credencial `Usami Token`: um envia
`{ type: 'proactive_say', message }` e o outro `{ type: 'gesture', name }`, porque
a fila aceita **um** comando por POST e exige `type`.
Aprovar técnica (proposta → validada) continua sendo decisão de uma PESSOA.

Regra de escrita no vault: automações só gravam nas pastas de automação
documentadas. Em `vault/.../Usami/Inbox/Automations/` mora um `README.md` com a
regra da arquitetura (`usami/docs/architecture.md`). O `Inbox/Responses/` é usado
apenas pelo workflow de respostas, conforme já estabelecido.

## Usami - Receber Respostas
- **Gatilho:** webhook `POST /webhook/usami-events` (a Usami dispara a cada `POST /api/chat`).
- **Faz:** valida o corpo (exige `reply` ou `userMessage`; senão lança erro) e monta uma nota Markdown.
- **Escreve em:** `Inbox/Responses/` (caminho do container `/files/obsidian/usami/Inbox/Responses/`), um arquivo por evento `{{timestamp}}-{{slug}}.md`.
- **Idempotência:** não é por lote — cada webhook gera arquivo com timestamp; a chave anti-colisão é o próprio timestamp + slug da mensagem.
- **Contrato do evento (medido em 2026-10-27 num `POST /api/chat` real, exec 15):** o corpo chega como `{ event: 'usami.response', timestamp, userMessage, reply, gesture, actions[], emotion: { emotion, confidence, intensity, facs, source }, affinity: { affinity, stage }, source }`. Atenção aos nomes: a emoção vem em `emotion.emotion` e o score em `affinity.affinity` — **não** existem `emotion.label` nem `affinity.score`. O nó lia esses nomes inexistentes e gravava `emotion_label: "neutral"` / `affinity_score: 0` mesmo com o corpo trazendo `happy`/0,95 e afinidade 10. Corrigido para aceitar as duas formas (default quando o campo vem ausente ou `null`: `neutral`/0/`desconhecido`) e revalidado pelo replay do mesmo payload na webhook de produção (exec 16): `emotion_label: "happy"`, `affinity_score: 10`, nota de 522 B escrita. Versão ativa: `78e256f3-62d3-4d91-9319-cd7f629bc52c`.
- **Latência (fora do escopo deste workflow; medição corrigida em 2026-10-27):** a leitura antiga estava errada por construção — cada `mark()` **atribuía** apenas o vão desde o mark anterior, e a espera do provedor não caía em nenhuma etapa, daí o `totalMs=21916` com `llmMs: 0`. O `timeStage` agora envolve as chamadas reais. Dois turnos medidos no caminho real depois da correção (API reiniciada, fonte `nvidia`): `llm=15957 animacao=0 contexto=2 total=15964` e `llm=18510 animacao=0 contexto=4 total=18519`. Sobre `animationMs=0`: é o pós-processamento síncrono entre o provedor responder e o `mark('animation')`, **não** uma afirmação sobre o refino em background — no segundo turno (com `capabilities` no corpo) o modelo já devolveu uma ação, a guarda `!actions.length` ficou falsa e o refino não disparou. Quando o forçamos no mesmo catálogo, com o planner real: `queuedMs=21786` para 3 ações na fila — custo que nunca entra em `animationMs`/`totalMs` porque o fire não é aguardado. O webhook continua fire-and-forget e não contribui para o total.
- **Estado da gravação (2026-10-27): gravando, reparo aplicado e validado nos dois sentidos.** Exec 13 (`POST /webhook/usami-events` sintético, modo `webhook`): os quatro nós passaram e a nota de 486 B foi escrita em `/files/obsidian/usami/Inbox/Responses/2026-10-27T15-00-00-000Z-teste-de-gravacao-apos-corrigir-a-allowlist.md`. A sonda (exec 12 e 14) relê esse arquivo do disco e lê `Inbox/Automations/*.md` normalmente. Antes do reparo: exec 8 (sintética) e 9 (`POST /api/chat` real) montaram a nota nos três primeiros nós (474 B / 528 B) e morreram no último com `ENOENT ... realpath '/files/obsidian/usami/Inbox/Responses'` — a pasta não existia; criada a pasta, a exec 10 mudou para `Access to the file is not allowed.`
- **Causa (fechada): separador errado em `N8N_RESTRICT_FILE_ACCESS_TO`.** O n8n lê a variável com `restrictFileAccessTo.split(';')` (`getAllowedPaths`, em `packages/core/src/execution-engine/node-execution-context/utils/file-system-helper-functions.ts`) e a doc de hosting diz "semicolon-separated list". O valor vivo usa **vírgula** (`/files/obsidian/usami,/home/node/vault/Inbox/Automations`), então a allowlist passa a ter **uma única entrada inválida** — a string inteira — e nenhum arquivo do vault era acessível, não só o `Inbox/Responses`.
  - Versão apurada no host: imagem `docker.n8n.io/n8nio/n8n:latest`, `n8n --version` = **2.44.6**. Conferido no fonte da tag exata (`n8n@2.44.6`) e em `2.0.0`, `2.30.0`, `2.40.0` e `1.113.0`: todos separam por `;`. Ou seja, a vírgula nunca foi lida — não é regressão de versão.
  - Hipótese de dono/modo foi **descartada com evidência** antes disso: `Automations` é `drwx------ luke luke` (700) e gravava, `Responses` é `drwxr-xr-x luke luke` (755), mesma propriedade.
  - Evidência decisiva, sem escrever nada: sonda read-only (`Usami - Sonda Permissao de Arquivos`, `NLs9mCXGphBz3X59`, execução 11) — `read` de `/files/obsidian/usami/Inbox/Automations/*.md`, a pasta das gravações de sucesso de 24/10, devolve o mesmo `Access to the file is not allowed.`. As gravações antigas são pré-recreate de 25/10 (o espelho desta stack entrou no git em 25/10, commit `7496140`), então a restrição provavelmente chegou junto com aquele recreate.
  - **Reparo (aplicado em 2026-10-27 pelo operador no host):** `N8N_RESTRICT_FILE_ACCESS_TO: /files/obsidian/usami;/home/node/vault/Inbox/Automations` em `~/n8n/docker-compose.yml` e `docker compose up -d`. O espelho versionado deste repositório está corrigido com o mesmo valor.
  - **Risco operacional (fechado em 2026-10-27):** a imagem estava em `latest`, então qualquer recreate trocava o binário do n8n em silêncio — inclusive o comportamento de path. O espelho está pinado em `docker.n8n.io/n8nio/n8n:2.44.6` (commit `ef80de0`); qualquer upgrade daqui pra frente é decisão explícita.
- **Sonda de permissão:** `Usami - Sonda Permissao de Arquivos` (`NLs9mCXGphBz3X59`) **não aparece na instância** (a busca por "Sonda" devolve 0 resultados e ela não está entre os 7 workflows ativos). Era só diagnóstico, com `operation: read` em três caminhos do vault e `onError: continueRegularOutput`; cumpriu o papel (execs 11/12/14) e não deve ser usada como referência.
- **Rotas para criar/ajustar a pasta, e o que falha:** `n8n-nodes-base.executeCommand` **não existe nesta instância** (`validate_workflow`: `Unrecognized node type`; `search_nodes`: sem resultado) — o nó está desabilitado no deploy, então não há shell pelo n8n. `ssh` para `10.42.0.1` está com conexão recusada em 22/2222/22022 (5678 responde: host de pé, sshd não). Restou o caminho de fora (host/`docker exec`).
- **Estado na instância do Nyarch:** o workflow está **ativo** (`bJJ17mDS2I5r4De9`), então cada `POST /api/chat` da Usami já gera nota em `Inbox/Responses/`. O envio é fire-and-forget: se o n8n estiver fora, o chat não sente e a nota simplesmente não nasce.
- **Vault local:** o vault de verdade é `A:/Usami Project/vault/Usami Obsidian/Usami` (tem `.obsidian/`); `A:/Usami Obsidian` não existe. A pasta `Inbox/Responses/` já foi criada no vault real.

## Usami - Obsidian Inbox (e variante `(local)`)
- **Gatilho:** webhook `POST /webhook/usami-inbox`.
- **Faz:** valida `title`/`content` e monta nota de evento da automação.
- **Escreve em:** `Inbox/Automations/{{timestamp}}-{{slug}}.md`. As duas variantes diferem só pelo caminho do vault montado em cada stack: `Usami - Obsidian Inbox.json` usa `/files/obsidian/usami/Inbox/Automations/` (stack do notebook Nyarch/Linux) e `Usami - Obsidian Inbox (local).json` usa `/home/node/vault/Inbox/Automations/` (stack local deste `docker-compose.yml`, que monta `vault/Usami Obsidian/Usami` em `/home/node/vault`).
- **Idempotência:** um arquivo por evento via timestamp + slug.

## Usami - Revisao de Tecnicas (agendado)
- **Gatilho:** Schedule Trigger diário às **04:00** (`triggerAtHour: 4`; o fuso efetivo é o `GENERIC_TIMEZONE: America/Sao_Paulo` do compose — a instância não define `settings.timezone`). Nada de intervalo curto.
- **Nós:** Schedule → HTTP Request → Code (filtrar/montar nota) → If (API respondeu?) → [falha] Stop and Error · [sucesso] Convert to File → Read/Write File → NoOp.
- **HTTP Request:** `GET http://10.42.0.1:8787/api/techniques` (bridge do Nyarch → API da Usami no Windows) com header `Origin: http://127.0.0.1:5173` (esse valor está na allowlist `DEFAULT_ALLOWED_ORIGINS` de `usami/server.mjs`; a rota devolve 403 sem origem permitida). Sem credencial: os dois GETs de leitura (`/api/techniques`, `/api/technique?id`) ficam só com a borda de origem. Timeout curto de **5000 ms** e `retryOnFail` com **`maxTries: 2`** (1 retry no máximo), para não martelar um servidor que está fora. `continueOnFail: true` — o erro vira um item com `error`, tratado de forma explícita pelo nó Code.
- **Leitura da resposta:** o item de `GET /api/techniques` traz `{id, name, status, confidence, motions, sources}` (via `summarizeTechnique`); a lista **não** expõe `summary`/`tags`/`createdAt` — esses campos só aparecem no detalhe `GET /api/technique?id=...`. Por isso o filtro usa `status`, `confidence` e `sources`.
- **Filtro de revisão humana:** `status === 'proposta'` **e** `confidence < 0.8` (limiar explícito `THRESHOLD` no nó Code) **e** `sources` não-vazio. Só isso vai para a nota.
- **Escreve em:** `Inbox/Automations/technique-review-<AAAA-MM-DD>.md`, no caminho do container `/files/obsidian/usami/Inbox/Automations/` (bind `/dados/obsidian/usami` do Nyarch). A nota lista o que espera aprovação e traz a instrução explícita de que a aprovação (proposta → validada) é feita por **PESSOA** na API da Usami — o workflow nunca chama endpoint de escrita nem altera status, e não contém comando de movimento.
- **Idempotência (mecanismo exato):** chave determinística de lote = **data local `AAAA-MM-DD`** no nome do arquivo, então rodar duas vezes no mesmo dia **sobrescreve a mesma nota** (sem nota nova/duplicada). Dentro do lote, dedupe por **`id|status`** com um `Set`: a mesma técnica não é listada duas vezes. O conteúdo é um snapshot recalculado da API a cada execução; ignorando `generated_at`, duas execuções no mesmo dia produzem corpo idêntico.
- **Se a API estiver fora:** o HTTP Request falha (retry único esgotado) e o item com `error` faz o If mandar para o nó **Stop and Error** com mensagem `API da Usami indisponivel: ... Nenhuma nota gravada.`. O workflow falha de forma visível e **não grava nota parcial** (o ramo de escrita não é alcançado). Idem se a resposta vier em formato inesperado (sem `techniques` nem `total`).
- **Custo/segurança:** só a rede do link (`10.42.0.0/24`), sem credencial, sem serviço externo, sem node que envie dados para fora da máquina. Não abre nem expõe porta.

## Validação registrada
- Tipos e parâmetros dos nós conferidos com o MCP do n8n em modo leitura (`get_node_types`, versão estática): `scheduleTrigger` (rule.interval `field:days`, `triggerAtHour`), `httpRequest` v4 (`headerParameters`, `options.timeout`, `retryOnFail`/`maxTries`/`continueOnFail` como propriedades do nó), `if` v2, `convertToFile` (`toText`), `readWriteFile` (`write`), `stopAndError`, `noOp`.
- `validate_workflow` do MCP exige código no formato n8n Workflow SDK, não o export JSON versionado; por isso não foi usado contra este arquivo. A lógica do nó Code foi exercitada localmente (Node) para sucesso, falha de API e idempotência.

## Usami - Google Assistant (desacoplamento Fase 1)
- **Arquivo:** `Usami - Google Assistant.json` (no export vem `active: false`; na instância do Nyarch está **ativo**, `2Boo8f2pnVgL5GRi`).
- **Gatilho:** Webhook `POST /webhook/usami-google`.
- **Papel:** Centraliza as integrações com Google Workspace (Gmail e Google Calendar) gerenciando OAuth2 nativamente no n8n.
- **Entrada:** `{ action: 'list_emails' | 'list_events', params: { query, maxResults, timeMin } }`.
- **Saída:** JSON padronizado com `{ ok: true, action: string, count: number, messages: [...] }` ou `{ ok: true, action: string, count: number, events: [...] }`.
- **Benefício:** Elimina a necessidade de bibliotecas e refresh tokens manuais de OAuth no Node.js da Usami.

## Usami - Daily Briefing (agendado - Fase 2)
- **Arquivo:** `Usami - Daily Briefing (agendado).json`.
- **Gatilho:** Schedule Trigger diário às **07:30** (`America/Sao_Paulo`).
- **Nós:** Schedule → HTTP Request (Open-Meteo Clima) → Code (Formatar Nota de Briefing) → Convert to File → Read/Write File (`Inbox/Automations/briefing-AAAA-MM-DD.md`) → HTTP Request (`POST /api/n8n/command`, fala) → HTTP Request (`POST /api/n8n/command`, gesto).
- **Papel:** Gera o resumo matinal no Obsidian com dados de clima e foco do dia antes do usuário abrir o app e enfileira uma saudação com gesto de aceno (`wave`).
- **Contrato da fila (corrigido em 2026-11-17):** `/api/n8n/command` aceita **um** comando por POST e exige `type`; o corpo antigo (`{ text, gesture }`) respondia 400. Agora são dois nós: `{ type: 'proactive_say', message: proactiveSay }` (timeout 10 s) e `{ type: 'gesture', name: 'wave' }`. Validado na instância com execução manual (exec 16413): `ok:true` nos dois comandos. O publish de 2026-11-17 (`5b6bf120-...`) substituiu a versão ativa `8be67225-...`, que gravava em `$env.USAMI_VAULT_PATH/Inbox/Automations` e chamava `127.0.0.1:8787`.
- **Segurança:** Apenas usa a API pública e aberta do Open-Meteo (zero chave/custo) e grava estritamente em `Inbox/Automations/`.

## Usami - Consolidacao Noturna (agendado - Fase 4)
- **Arquivo:** `Usami - Consolidacao Noturna (agendado).json`.
- **Gatilho:** Schedule Trigger diário às **04:00** (`America/Sao_Paulo`), mesmo horário do `Revisao de Tecnicas`.
- **Nós:** Schedule → HTTP Request (`POST http://10.42.0.1:8787/api/memory/maintenance`) → If (`ok === true`) → NoOp / Stop and Error.
- **Papel:** Executa em segundo plano a sumarização dos dias anteriores, consolidação de fatos duráveis na memória, refresh do índice FTS5 e contagem/manutenção do LanceDB local, aliviando o loop do chat de tarefas de I/O pesado.
- **Segurança:** Protegido por `x-usami-token` (credencial `Usami Token` no n8n) e allowlist de Host/Origin.
- **Observação:** na instância os nós têm sufixo `1` nos nomes (`Disparo Noturno (04:00)1` etc.); o export usa os nomes sem sufixo.

## Usami - Relatorio Financeiro (agendado - Fase 3)
- **Arquivo:** `Usami - Relatorio Financeiro (agendado).json`.
- **Gatilho:** Schedule Trigger diário às **08:00** (`America/Sao_Paulo`).
- **Nós:** Schedule → HTTP Request (`POST http://10.42.0.1:8787/api/agent/tools/finance.transactions`, credencial `Usami Token`) → Code (Processar Extrato e Alertas) → Convert to File → Read/Write File (`/files/obsidian/usami/Inbox/Automations/financas-AAAA-MM.md`) → HTTP Request (`POST /api/n8n/command`, fala) → HTTP Request (`POST /api/n8n/command`, gesto).
- **Papel:** Consulta transações e saldos do Open Finance (Pluggy / Belvo) em modo somente leitura via API de ferramentas da Usami, consolida em relatório mensal no Obsidian e avisa com gesto `surprised` se alguma transação de saída passar de R$ 500, ou com gesto `nod` na atualização normal.
- **Contrato da fila (corrigido em 2026-11-17):** dois nós, como no briefing: `{ type: 'proactive_say', message: proactiveSay }` (timeout 10 s) e `{ type: 'gesture', name: <surprised|nod> }`. Validado com execução manual (exec 16401): `ok:true` nos dois. Sem credenciais do Open Finance a consulta responde 404 e a nota sai como "indisponível" (o nó Code já trata isso).
- **Segurança:** Somente leitura, token protegido e escrita exclusiva em `Inbox/Automations/`.

## Observabilidade e Bot Discord da Usami (modulares)

Os fluxos são divididos por espaço/domínio e utilizam o PostgreSQL de aplicação (`n8n-usami-postgres`, banco `usami`, tabelas em `infra/n8n-local/nyarch/init-db.sql`) e o micro-bridge `n8n-discord-bridge` (`infra/n8n-local/nyarch/discord-bridge/`).

Credenciais necessárias no n8n após importar os JSONs:
- **`Usami Postgres`**: Postgres (`host: postgres`, `database: usami`, `user: usami`, senha `USAMI_POSTGRES_PASSWORD`).
- **`Discord Bot Token`**: HTTP Header Auth (`Name: Authorization`, `Value: Bot <DISCORD_BOT_TOKEN>`).
- **`n8n API Key`**: HTTP Header Auth (`Name: X-N8N-API-KEY`, `Value: <chave gerada em Settings -> n8n API>`).
- **`Usami Token`**: já existente (`x-usami-token`).

### Espaço A — Observabilidade
- **`Usami - Observabilidade Ingest.json`** (`POST /webhook/usami-observability`): recebe em *fire-and-forget* chamadas HTTP `/api/*`, chamadas LLM/tools (`usami/src/telemetry.js` + `usami/src/n8n-observability.js`) e erros do `discord-bridge`, sanitiza tokens/chaves sem registrar texto privado de chat, grava em `obs_api_logs`/`obs_error_logs` e envia alerta imediato para `#usami-erros` (`DISCORD_OBS_ERRORS_CHANNEL_ID`).
- **`Usami - Monitor de Erros e Execucoes n8n.json`** (`Error Trigger` + `Schedule` 15 min): captura falhas de qualquer workflow (defina este workflow como *Error Workflow* nas configurações dos demais) e coleta periodicamente `/api/v1/executions`, gravando em `obs_error_logs` e `obs_n8n_executions` com alerta em `#usami-erros`.
- **`Usami - Relatorio Observabilidade (agendado).json`** (`Schedule` diário às 23:50): consolida métricas das últimas 24h (chamadas API, latência média/p95, LLM por modelo, erros e execuções n8n), grava `/files/obsidian/usami/Inbox/Automations/observability-AAAA-MM-DD.md` e posta resumo em `#usami-observabilidade` (`DISCORD_OBS_SUMMARY_CHANNEL_ID`).

### Espaço B — Bot Discord da Usami (inspirado em `Rukafuu/LiraVtuber`)
- **`Usami - Discord Chat Companion.json`** (`POST /webhook/usami-discord-chat`): responde menções `@Usami`, DMs e o canal `#usami-chat`. Suporta texto, leitura de arquivos anexados (`.txt`, `.py`, `.json`, etc.) e visão multimodal para imagens e vídeos via `POST http://10.42.0.1:8787/api/media/analyze` (ou `/api/chat` para conversas puramente textuais), com fallback amigável caso o PC Windows esteja offline.
- **`Usami - Discord Engajamento e Utilidades.json`** (`POST /webhook/usami-discord-community` + `Schedule` 09:00 e 5 min): boas-vindas (`guildMemberAdd`), sistema AFK automático, aniversários (`/aniversario` + parabéns às 09:00), Notepad (`/nota`), sugestões com votação (`/sugestao`), sorteios (`/sorteio` + sorteio automático de vencedores), estatísticas (`/stats`) e resumo rápido de observabilidade (`/obs`).
- **`Usami - Discord Moderacao e Estrutura.json`** (`POST /webhook/usami-discord-mod`): Auto-Mod (links de convite, menções em massa, golpes comuns + log em `#mod-logs`), Reaction Roles (`/reactionrole` + reações), Tickets de suporte privados (`/ticket`), Sticky Messages (`/sticky` + repostagem automática) e comandos customizados (`/cmd` + `!nome`).
- **`Usami - Discord Usami Principal.json`** (`POST /webhook/usami-discord-usami`): comando unificado `/usami` com 6 subcomandos:
  - `/usami chat <mensagem> [thread:true]` — conversa com a Usami (opcionalmente cria thread para conversas longas)
  - `/usami status` — mostra Rich Presence, humor, afinidade, modo sono e estatísticas do servidor
  - `/usami memoria ver [chave]` / `/usami memoria limpar <chave>` — gerencia memória de longo prazo por servidor
  - `/usami humor ver` / `/usami humor definir <humor>` — define humor: `happy` (Alegre ✨), `tsundere` (Tsundere 💢), `surprised` (Surpresa 💡), `thinking` (Pensativa 💭), `neutral` (Neutro), `sad` (Triste), `angry` (Brava 💢)
  - `/usami afinidade ver` / `/usami afinidade definir <estagio>` — define estágio: `Conhecida` 🤝, `Amiga` 💜, `Amiga Íntima` 💕, `Confidente` 🤫, `Alma Gêmea` 💖
  - `/usami sono ativar` / `/usami sono desativar` — ativa modo companhia para dormir por 30 minutos (mensagens suaves periódicas)

## Usami - Busca e Matching de Vagas (agendado)
- **Arquivo:** `Usami - Busca e Matching de Vagas (agendado).json` (id `UBTFGRMidGiVdbpu`).
- **Gatilho:** Schedule Trigger diário às **09:00** (`America/Sao_Paulo`).
- **Nós:** Schedule → HTTP Request (`GET https://remoteok.com/api`) → Code (`Extrair Vagas`) → Code (`Calcular Score ATS`) → Code (`Filtrar Score >= 10`) → Data Table (`Filtrar Vagas Ineditas` via `rowNotExists` em `candidatura_id`) → Data Table (`Registrar na Tabela`, status `Nova`) → Wait (5s) → Code (`Preparar Mensagem Discord`) → HTTP Request (`Notificar Discord` via Webhook).
- **Papel:** Varre vagas remotas públicas, calcula pontuação de afinidade ATS para o perfil de Engenharia de Software / IA / Backend, desduplica contra a Data Table local `controle_candidaturas` para reter apenas vagas inéditas, cadastra com status `Nova` e envia alerta formatado no Discord com link direto para candidatura manual.
- **Webhook do Discord:** o nó `Notificar Discord` usa `{{ $env.DISCORD_VAGAS_WEBHOOK_URL }}`. Defina a URL em `~/n8n/.env` e recrie o container (`docker compose up -d n8n`); variável vazia faz o nó falhar com erro visível na observabilidade, em vez de sumir em silêncio.
- **Segurança & Idempotência:** Não realiza submissões/candidaturas automáticas (apenas triagem e notificação informativa). Idempotência garantida pelo nó nativo `rowNotExists` que impede alertas repetidos ou re-cadastro de IDs já processados.

