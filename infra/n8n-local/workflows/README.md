# Workflows

Exports JSON dos workflows do n8n local (controle de versão). Cada arquivo é o
export de um workflow; importar no editor do n8n (Workflows → Importar) para usar.
Nada aqui é publicado/ativado automaticamente — o arquivo `Usami - Revisao de
Tecnicas (agendado).json` vem com `active: false` de propósito.

## Instância autoritativa e URL real

A instância que vale é a do **Nyarch** (`http://10.42.0.1:5678`; é para onde o
MCP aponta e é o `WEBHOOK_URL` do `nyarch/docker-compose.yml`). A stack local
deste diretório (`docker-compose.yml`, porta `127.0.0.1:5678`) está **desligada**
e não recebe nada hoje. Consequências práticas:

- O `usami/server.mjs` resolve `N8N_WEBHOOK_URL` do environment ou do
  `A:/Usami Project/.env` (nada popula `process.env` a partir desse arquivo).
  Sem essa linha no `.env`, o default `http://localhost:5678/...` vence e o
  evento `usami.response` é enviado para lugar nenhum em silêncio.
- Export versionado ≠ publicado. Confirme na instância (`search_workflows`) antes
  de confiar num workflow. Estado atual: `Usami - Obsidian Inbox`
  (`NnWm0PoOh4IBjtQS`) ativo e publicado; `Usami - Receber Respostas`
  (`bJJ17mDS2I5r4De9`) **publicado e ativo** desde 2026-10-26 (webhook
  `POST /webhook/usami-events` responde 200);
  `Usami - Revisao de Tecnicas (agendado)` segue ausente (ver bloqueio abaixo).
- A publicação é feita por **código SDK** (`validate_workflow` →
  `create_workflow_from_code`), não pelo export JSON. O export de
  `Usami - Receber Respostas` foi regerado a partir do que a instância devolveu,
  então os `id`/`webhookId`/UUIDs de nó dele são os reais; importar aquele JSON
  criaria uma segunda cópia com outros ids.

## Bloqueio do "Revisao de Tecnicas" nesta instância

O nó HTTP do export chama `GET http://127.0.0.1:8787/api/techniques`, escrito
para a stack local do Windows. Na stack do Nyarch isso é o próprio container
(como `usami/server.mjs:1807` escuta em `127.0.0.1:8787` **somente no loopback
do Windows**), e o compose do Nyarch não define `extra_hosts` nem
`network_mode: host` — então `host.docker.internal` também não resolve. A
Windows está em `10.42.0.91` no mesmo link. Publicar esse workflow como está
produziria nota de falha todo dia às 04:00. Precisa de uma decisão de exposição
(bind não-loopback da API, túnel SSH reverso + `extra_hosts`, ou rodá-lo na
stack local do Windows) antes de importar.

Regra de ouro: **o n8n não move o avatar.** Estes workflows só fazem tarefas
assíncronas/agendadas de casa (receber evento, listar/consolidar, escrever nota).
Nenhum deles emite comando de movimento, controla osso/frame-loop nem chama
`/api/n8n/command`. No máximo produzem tarefa/revisão para uma PESSOA executar.

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
- **Sonda de permissão:** `Usami - Sonda Permissao de Arquivos` (`NLs9mCXGphBz3X59`) existe só para diagnóstico, com `operation: read` em três caminhos do vault e `onError: continueRegularOutput` — não escreve nem apaga nada. Já cumpriu o papel (execs 11/12/14); pode ser arquivada quando o `Responses` não for mais assunto.
- **Rotas para criar/ajustar a pasta, e o que falha:** `n8n-nodes-base.executeCommand` **não existe nesta instância** (`validate_workflow`: `Unrecognized node type`; `search_nodes`: sem resultado) — o nó está desabilitado no deploy, então não há shell pelo n8n. `ssh` para `10.42.0.1` está com conexão recusada em 22/2222/22022 (5678 responde: host de pé, sshd não). Restou o caminho de fora (host/`docker exec`).
- **Estado na instância do Nyarch:** o workflow está **ativo** (`bJJ17mDS2I5r4De9`), então cada `POST /api/chat` da Usami já gera nota em `Inbox/Responses/`. O envio é fire-and-forget: se o n8n estiver fora, o chat não sente e a nota simplesmente não nasce.
- **Vault local:** o vault de verdade é `A:/Usami Project/vault/Usami Obsidian/Usami` (tem `.obsidian/`); `A:/Usami Obsidian` não existe. A pasta `Inbox/Responses/` já foi criada no vault real.

## Usami - Obsidian Inbox (e variante `(local)`)
- **Gatilho:** webhook `POST /webhook/usami-inbox`.
- **Faz:** valida `title`/`content` e monta nota de evento da automação.
- **Escreve em:** `Inbox/Automations/{{timestamp}}-{{slug}}.md`. As duas variantes diferem só pelo caminho do vault montado em cada stack: `Usami - Obsidian Inbox.json` usa `/files/obsidian/usami/Inbox/Automations/` (stack do notebook Nyarch/Linux) e `Usami - Obsidian Inbox (local).json` usa `/home/node/vault/Inbox/Automations/` (stack local deste `docker-compose.yml`, que monta `vault/Usami Obsidian/Usami` em `/home/node/vault`).
- **Idempotência:** um arquivo por evento via timestamp + slug.

## Usami - Revisao de Tecnicas (agendado)
- **Gatilho:** Schedule Trigger diário às **04:00** (fuso `America/Sao_Paulo`, no `settings.timezone`). Nada de intervalo curto.
- **Nós:** Schedule → HTTP Request → Code (filtrar/montar nota) → If (API respondeu?) → [falha] Stop and Error · [sucesso] Convert to File → Read/Write File → NoOp.
- **HTTP Request:** `GET http://127.0.0.1:8787/api/techniques` com header `Origin: http://127.0.0.1:5173` (esse valor está na allowlist `DEFAULT_ALLOWED_ORIGINS` de `usami/server.mjs`; a rota devolve 403 sem origem permitida). Timeout curto de **5000 ms** e `retryOnFail` com **`maxTries: 2`** (1 retry no máximo), para não martelar um servidor que está fora. `continueOnFail: true` — o erro vira um item com `error`, tratado de forma explícita pelo nó Code.
- **Leitura da resposta:** o item de `GET /api/techniques` traz `{id, name, status, confidence, motions, sources}` (via `summarizeTechnique`); a lista **não** expõe `summary`/`tags`/`createdAt` — esses campos só aparecem no detalhe `GET /api/technique?id=...`. Por isso o filtro usa `status`, `confidence` e `sources`.
- **Filtro de revisão humana:** `status === 'proposta'` **e** `confidence < 0.8` (limiar explícito `THRESHOLD` no nó Code) **e** `sources` não-vazio. Só isso vai para a nota.
- **Escreve em:** `Inbox/Automations/technique-review-<AAAA-MM-DD>.md` (caminho do container `/home/node/vault/Inbox/Automations/`, mesmo da stack local). A nota lista o que espera aprovação e traz a instrução explícita de que a aprovação (proposta → validada) é feita por **PESSOA** na API da Usami — o workflow nunca chama endpoint de escrita nem altera status, e não contém comando de movimento.
- **Idempotência (mecanismo exato):** chave determinística de lote = **data local `AAAA-MM-DD`** no nome do arquivo, então rodar duas vezes no mesmo dia **sobrescreve a mesma nota** (sem nota nova/duplicada). Dentro do lote, dedupe por **`id|status`** com um `Set`: a mesma técnica não é listada duas vezes. O conteúdo é um snapshot recalculado da API a cada execução; ignorando `generated_at`, duas execuções no mesmo dia produzem corpo idêntico.
- **Se a API estiver fora:** o HTTP Request falha (retry único esgotado) e o item com `error` faz o If mandar para o nó **Stop and Error** com mensagem `API da Usami indisponivel: ... Nenhuma nota gravada.`. O workflow falha de forma visível e **não grava nota parcial** (o ramo de escrita não é alcançado). Idem se a resposta vier em formato inesperado (sem `techniques` nem `total`).
- **Custo/segurança:** só localhost, sem credencial, sem serviço externo, sem node que envie dados para fora da máquina. Não abre nem expõe porta.

## Validação registrada
- Tipos e parâmetros dos nós conferidos com o MCP do n8n em modo leitura (`get_node_types`, versão estática): `scheduleTrigger` (rule.interval `field:days`, `triggerAtHour`), `httpRequest` v4 (`headerParameters`, `options.timeout`, `retryOnFail`/`maxTries`/`continueOnFail` como propriedades do nó), `if` v2, `convertToFile` (`toText`), `readWriteFile` (`write`), `stopAndError`, `noOp`.
- `validate_workflow` do MCP exige código no formato n8n Workflow SDK, não o export JSON versionado; por isso não foi usado contra este arquivo. A lógica do nó Code foi exercitada localmente (Node) para sucesso, falha de API e idempotência.
