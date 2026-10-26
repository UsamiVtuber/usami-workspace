# Workflows

Exports JSON dos workflows do n8n local (controle de versão). Cada arquivo é o
export de um workflow; importar no editor do n8n (Workflows → Importar) para usar.
Nada aqui é publicado/ativado automaticamente — o arquivo `Usami - Revisao de
Tecnicas (agendado).json` vem com `active: false` de propósito.

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
