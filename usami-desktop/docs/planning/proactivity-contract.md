# Contrato de proatividade do desktop

**Status:** desenho para aprovação. Nada aqui está implementado.
**Escopo:** definir o contrato entre `usami-desktop/` e a stack local
(API em `usami/` + n8n em `infra/n8n-local/`) para lembretes, saudação por
horário e resumo do dia.
**Fora de escopo:** criar/alterar workflow, tocar em `docker-compose`,
volumes, containers ou `vault/`, abrir porta, publicar serviço, alterar túnel,
mexer em credenciais.

Premissa que manda em tudo: **o desktop não é servidor**. Ele só consome o
backend local `http://127.0.0.1:8787`; qualquer entrega é *pull* a partir da
API. Nada de webhook para o desktop, porta nova ou exposição externa.

---

## 0. O que já existe (verificado em 2026-11-08)

| Peça | Fato | Evidência |
| --- | --- | --- |
| API | Escuta só em `127.0.0.1:8787` | `usami/server.mjs` listen local (linha ~2419) |
| Borda de segurança | Host/Origin fechados; rotas sensíveis exigem `x-usami-token` e falham fechado (503 sem token no servidor, 401 com token errado/ausente) | `usami/src/api-security.js` (`assertLoopbackOrigin`, `assertApiToken`), `usami/server.mjs` |
| Sonda real | `GET /api/proactive/state` → **401** `{"error":"Token ausente ou inválido."}`; `GET /api/n8n/commands` → **401** | curl local, 2026-11-08 |
| Fila de comandos | Em memória, **por cliente** (`x-usami-client`), entrega *at most once*, entregue-e-não-confirmado podado em 120 s, pendente sem dono expira em 5 min, teto 50/cliente | `usami/src/n8n-queue.js` (`createCommandQueue`, linhas ~18-21, 115-144) |
| Fila — semântica | “Repetir um gesto/fala já aplicado é pior que perdê-lo” (decisão explícita) | `usami/src/n8n-queue.js` comentário de arquitetura no topo |
| Motor proativo | Existe com idle de 3 min, DND 23:30–08:30, afinidade ≥ 20, no máximo 1 não respondida | `usami/src/proactive-engine.js` (`createProactiveEngine`, `checkAndTrigger`) |
| Motor proativo — estado | **Dormente**: não há `proactiveEngine.start()` em lugar nenhum do código; só responde a `POST /api/proactive/trigger` (tokenizado) | grep + `usami/server.mjs` rota `/api/proactive/trigger` |
| Conteúdo local | `getTimeSlot` (8 faixas: alvorada/manhã/almoço/tarde/entardecer/noite/tarde da noite/madrugada) e cenas diárias determinísticas | `usami/src/scenes.js` (`getTimeSlot`), `usami/src/scenes-data.js` |
| Geração de fala | O engine é instanciado **sem router** → `generateProactiveSpeech` cai no fallback determinístico (nenhuma chamada a provedor externo) | `usami/server.mjs` instanciação de `createProactiveEngine` (linha ~175-186), `proactive-engine.js` |
| Desktop | Fala só `/api/chat`, `/api/tts`, `/api/stt`; **não** faz poll de fila nem envia token | `usami-desktop/src/api.rs` (`send_chat`, `stream_tts`, `transcribe_audio`) |
| Custo do poll atual da web | 2 s contínuo, ~1800 GETs/h; já registrado como candidato a long-poll/SSE | `usami/src/Usami.jsx:1877`, `usami/docs/auditoria-2026-11-08.md` (R6) |
| n8n autoritativo | Instância do Nyarch (`10.42.0.1:5678`); ativos: `Usami - Receber Respostas` (`bJJ17mDS2I5r4De9`) e `Usami - Obsidian Inbox` (`NnWm0PoOh4IBjtQS`); sonda inativa | MCP `search_workflows`, 2026-11-08 |
| Alcance n8n → API | **Bloqueado hoje**: a API só escuta loopback do Windows e o n8n roda no container do Nyarch | `infra/n8n-local/workflows/README.md` §“Bloqueio do Revisao de Tecnicas”; `nyarch/README.md` §Exposição |
| Persistência local da API | JSON atômico em `usami/data/` e SQLite via `node:sqlite` (`vault-index.sqlite`, `techniques-index.sqlite`) | `usami/src/vault-index.js:21-25`, `usami/data/` |

Consequência direta: **nenhum evento proativo chega a um cliente hoje**. O motor
existe, mas não tem relógio; o frontend não chama `/api/proactive/*`
(verificado por grep em `usami/src/`), o desktop não consome evento nenhum e o
n8n não consegue POSTar na API. O contrato precisa nascer sabendo disso —
inclusive porque a opção “n8n manda evento” não é grátis.

---

## 1. Transporte

### 1.1 Opções

| Opção | Como seria | Custo | Modo de falha | Veredito |
| --- | --- | --- | --- | --- |
| **A. Polling curto na API local** (recomendada) | `GET /api/proactive/events` a cada 15 s, jitter ±3 s, poll imediato no boot e no foco da janela; backoff 30/60 s em falha | ~240 GETs/h idle (≈ 7,5× menos que os 1800/h da web); resposta de poucos bytes; um timer tokio já no padrão do app | API fora → erro de conexão silencioso (vira status discreto e backoff); evento não se perde porque o servidor guarda não confirmados; latência de até 15 s (irrelevante para lembrete/saudação) | **Adotar** |
| **B. SSE** (`text/event-stream`) | `GET /api/proactive/stream` com heartbeat de 20 s, `Last-Event-ID` para replay | 1 conexão por desktop; 3 heartbeats/min; exige `res.write` contínuo, limpeza de cliente lento e reconexão com backoff | TCP meio-aberto só é percebido pelo heartbeat; servidor reiniciou → gap; sem replay por cursor o evento do intervalo se perde; mais superfície de teste no servidor HTTP minimalista | Fase 2, **mesmo schema**, só se a latência de 15 s incomodar |
| **C. Fila no Postgres** | Tabela `proactive_events` no Postgres do n8n, desktop lendo direto | Publicar a porta do Postgres no link ou pôr credencial no executável | Exposição de rede (proibida por premissa) **ou** segredo dentro de um zip público; acopla o app ao banco interno do n8n; n8n e API continuam em máquinas diferentes | **Rejeitar** |
| **C′. Arquivo-caixa no vault** (ponte, não transporte) | n8n grava `reminders.pending.json` no vault via SMB (tmp+rename atômico); a API local ingere via timer/GET resiliente | Zero porta nova; reusa o mount `/mnt/usami-automations` que já existe | Depende do SMB montado; latência do ciclo do n8n (minutos) | Usar **só como ponte n8n → API**, nunca como canal desktop |

### 1.2 Recomendação

**Opção A** — polling curto em um inbox novo da API local, com confirmação por
id. Motivos, em ordem:

1. não abre nada: mantém `127.0.0.1` e a borda Host/Origin/token que já existe;
2. reusa o vocabulário que o projeto já domou (fila por cliente + `ack`), mas
   com semântica **at-least-once** para lembrete — o oposto do at-most-once da
   fila de gestos, e por isso **em store separado**;
3. tolera fechar/abrir o app e reiniciar a API sem perder evento dentro da
   validade;
4. é o caminho que a própria auditoria já apontou como próximo passo (R6),
   sem obrigar a migrar a web junto.

SSE fica documentado como evolução: se um dia o polling incomodar, o endpoint
de stream pode entregar **o mesmo payload** e o desktop troca só o laço de
transporte. O schema e o ack não mudam.

**Não misturar com `/api/n8n/commands`.** Aquele endereço é para comando de
avatar (`gesture`, `proactive_say`, morph), com entrega at-most-once; um
lembrete que some em 120 s é um bug de produto, não uma decisão aceitável.
O desktop pode continuar sem consumir a fila de comandos.

---

## 2. Schema do evento

### 2.1 Evento (corpo)

```json
{
  "id": "evt_01JCM4Z8Q3K7T2",
  "type": "reminder",
  "text": "Consultar o resultado do exame às 15h.",
  "priority": "normal",
  "createdAt": "2026-11-09T09:00:00-03:00",
  "notBefore": "2026-11-09T14:45:00-03:00",
  "expiresAt": "2026-11-09T23:59:59-03:00",
  "dedupeKey": "vault:lembrete:2026-11-09-exame",
  "source": "vault_ingest",
  "speak": true,
  "style": "casual",
  "emotion": "happy",
  "gesture": "nod",
  "payload": { "note": "Inbox/Automations/2026-11-09-exame.md" }
}
```

| Campo | Regra |
| --- | --- |
| `id` | Gerado **no servidor** (ULID/UUIDv7), imutável. Corpo do produtor nunca define `id` — mesma defesa de `queueAvatarCommand` (`server.mjs:118-133`). |
| `type` | `greeting` \| `reminder` \| `daily_digest` \| `idle_nudge`. Tipo desconhecido é descartado com 400. |
| `text` | Texto puro, 1–300 caracteres, sem Markdown/HTML. É o que vai para balão e, se `speak`, para o TTS. |
| `priority` | `low` \| `normal` \| `high`. Modula ordem e exceção de silêncio (ver §5). Não modula frequência. |
| `createdAt` | Carimbado pelo servidor. |
| `notBefore` | Instante mínimo de exibição (ISO com offset). Se chegar antes, o desktop retém. |
| `expiresAt` | Fim da validade. Depois disso a API não entrega e o desktop descarta. Obrigatório em `reminder` e `daily_digest`. |
| `dedupeKey` | Idempotência do produtor, escopo `(source, dedupeKey)`. Repetir a chave **atualiza** o evento pendente em vez de duplicar; repetir depois do ack, dentro de 24 h, **não recria** (responde `deduped: true`). Importante: correção de texto/horário de lembrete já exibido exige chave nova (ex.: `:v2`), pois chave repetida pós-ack é descartada pela tombstone. |
| `source` | `api_scheduler` \| `vault_ingest` \| `desktop` \| `chat`. |
| `speak` | `true` por padrão; `false` = só balão (ver §5, TTS). |
| `style` / `emotion` / `gesture` | Opcionais. `style` alimenta `/api/tts`; `emotion`/`gesture` só valem se estiverem no catálogo que o desktop anuncia em `api.rs:119-131`. Valor fora do catálogo é ignorado, nunca erro. |
| `payload` | Objeto pequeno (≤ 1 KB) para diagnóstico/produtor. O desktop **não** interpreta na v1. |

Limites: corpo do produtor ≤ 8 KB (mesmo teto de `readRequestBody` usado nas
rotas atuais), até 20 eventos por resposta, `acks` até 50 por POST.

### 2.2 Envelope de entrega

```json
{
  "events": [ /* eventos */ ],
  "serverTime": "2026-11-09T14:46:03-03:00",
  "cursor": "opcional, só otimização"
}
```

O cliente **não** usa `cursor` como fonte de verdade: ele recebe **todos os
não confirmados e não expirados** que já estão em `notBefore`. Assim um poll
perdido não perde evento; o `cursor` só reduz payload no futuro.

### 2.3 Endpoints propostos (API, ainda não implementados)

| Rota | Uso | Borda |
| --- | --- | --- |
| `GET /api/proactive/events` | desktop busca pendentes | Host/Origin + `x-usami-token`; `x-usami-client` recomendado (log) |
| `POST /api/proactive/events/ack` | confirma o que foi tratado | idem + `Content-Type: application/json` |
| `POST /api/proactive/events` | produtor cria evento (n8n local, ingest do vault, futuro UI) | idem |
| `GET /api/proactive/state` | já existe; continua read-only | já tokenizado |

Detalhe de borda: o `reqwest` não envia `Origin`; a API aceita ausência de
`Origin` quando o `Host` é loopback (`api-security.js:56-60`). O header
obrigatório no desktop é o **token** — sem ele, 401 (medido).

**Inbox único na v1.** O evento não é por cliente: o `ack` remove para todos os
consumidores e `x-usami-client` serve para log e para um fanout futuro. Com duas
instâncias do desktop abertas, as duas podem exibir o mesmo evento na mesma
janela de poll (corrida de até 15 s, aceita); a primeira que confirmar remove
para a outra.

**Heartbeat de atividade no mesmo GET.** O poll envia
`X-Usami-Activity: active|idle` (o desktop marca `active` se houve interação nos
últimos 60 s). A API chama `markUserActivity()` em `active` e usa esse relógio
para `idle_nudge`; sem isso o motor acharia o usuário ocioso só porque ele não
mandou mensagem no chat, e a API não ganha requisição nova por isso.

`ack` no formato novo:

```json
{ "acks": [ { "id": "evt_...", "outcome": "shown" } ] }
```

`outcome`: `shown` (exibido), `spoken` (falado), `suppressed` (silêncio/desligado,
não repetir) e `expired` (já venceu). “Agora não” **não** é ack: o desktop
apenas segura o evento na fila local e confirma quando exibir — assim, se o app
fechar no meio, o servidor reentrega na próxima sessão. O endpoint atual
`/api/n8n/commands/ack` (`{ids:[...]}`) fica intacto.

### 2.4 Máquina de estados (servidor)

```
pending  --(notBefore <= now)-->  due  --(GET)-->  delivered
delivered --(ack shown/spoken)--> acked (removido)
delivered --(sem ack 5 min, reminder)--> due (reentrega até expirar)
delivered --(sem ack 5 min, social, máx. 2)--> due; depois conta como entregue
pending|due|delivered --(expiresAt < now)--> expired (removido)
```

O teto de reentrega existe só para `greeting`/`idle_nudge` (evita insistência
quando o cliente está vivo e ignorando). `reminder` reentrega enquanto estiver
na validade: perder o lembrete é pior que repetir o GET.

- Store **persistente** em `usami/data/proactive-events.json` com escrita
  atômica (tmp+rename, o mesmo padrão de `memory.json`). Sobrevive a restart da
  API e não exige migração de schema. Se o volume justificar, a alternativa é
  uma tabela no SQLite local que a API já usa (`node:sqlite`); a decisão não
  muda o contrato HTTP.
- Poda a cada leitura/escrita; teto de 200 linhas (o mais antigo e já acked
  sai primeiro) e tombstones de `dedupeKey` limitados a 200 chaves / 24 h.
- `dedupeKey` repetida substitui o evento pendente (mesmo `id`), preservando
  `createdAt`; depois do ack, a tombstone impede recriação na janela de 24 h.
- **Correção de lembrete já exibido exige chave nova:** se uma nota no vault for
  editada para corrigir texto ou horário após o ack, reutilizar a mesma
  `dedupeKey` faz o servidor ignorar a alteração (`deduped: true`). O produtor
  deve anexar versão ou hash do conteúdo (ex.: `...-exame:v2`) quando o objetivo
  for reemitir um lembrete modificado.

### 2.5 Como o desktop evita repetir **e** perder

**Não repetir**
1. `id` é a chave de idempotência do consumidor. O desktop guarda um
   *seen-set* local (até 200 ids, FIFO com timestamp) em
   `%APPDATA%\Usami\proactivity.json` — sem dependência nova (basta
   `std::env::var("APPDATA")`).
2. O `id` entra no seen-set **no momento em que o balão aparece**, e o `ack`
   `shown` é enviado logo em seguida. Se o ack falhar, a redelivery em 5 min
   encontra o id no seen-set: não fala de novo, só reenvia o ack.
3. Ack é idempotente (id desconhecido → 200), como o `acknowledge` de hoje.
4. Falha de rede no meio do ack não gera fala duplicada pelo motivo acima.

**Não perder**
1. O servidor entrega por *não confirmado*, não por cursor; poll perdido não
   pula evento.
2. O evento só sai do store por `ack` ou por `expiresAt`. Fechar o app por
   minutos/horas dentro da validade não perde nada.
3. Primeiro poll é imediato no boot e no foco da janela — não espera o
   primeiro tick do timer.
4. Reentrega após 5 min cobre queda do app entre receber e confirmar; para
   `reminder` ela continua enquanto o evento estiver na validade.
5. Se a API reiniciar, o store persistido devolve o que estava `due`/`delivered`.

**Perda aceita (explícita):** evento cujo `expiresAt` passou enquanto o app
estava fechado. Um lembrete de ontem às 15h não deve chegar hoje às 9h.

### 2.6 Frequência e fila de apresentação no desktop

- Servidor aplica os tetos (§5.2) — o desktop não é a única barreira.
- Desktop mantém fila em memória (máx. 5). Exibe **um por vez**; `low` pode ser
  descartado se a fila encher.
- **Nunca interrompe** `is_busy` (chat em andamento), TTS tocando, chamada
  (`is_calling`), modo companhia (sono de 30 min) ou microfone aberto. O evento
  fica retido na fila local, **sem ack**, e é exibido quando o estado voltar a
  ocioso — até `expiresAt`. Se o app fechar nesse meio-tempo, o servidor
  reentrega na próxima sessão.
- `greeting`/`idle_nudge` respeitam cooldown local de 5 min entre falas.
  `reminder` é isento do cooldown (o horário é do lembrete), mas continua
  proibido de interromper fala/chat/chamada.
- **Trabalho em andamento fora deste contrato:** `usami-desktop/src/presence.rs`
  (ícone de bandeja, atalho global, sempre-no-topo e autostart) não está
  commitado em 2026-11-08. Duas consequências para cá: o poll nasce com o
  **processo**, não com a janela; e, com a janela oculta na bandeja, evento não
  é falado para uma sala vazia — fica retido (sem ack) até a janela aparecer,
  ou vira notificação de bandeja (decisão à parte).

---

## 3. Base no n8n

### 3.1 O que serve de base (citando os exports)

| Arquivo | O que aproveitar | Limite |
| --- | --- | --- |
| `infra/n8n-local/workflows/Usami - Revisao de Tecnicas (agendado).json` | Único padrão de **agendamento** do repositório: `Schedule Trigger` 04:00 `America/Sao_Paulo` → `HTTP Request` com timeout 5000 ms e `retryOnFail/maxTries: 2` → `Code` de filtro → `If` → ramo de erro `Stop and Error`; idempotência por chave de data no nome do arquivo | O HTTP aponta para `127.0.0.1:8787`, que, de dentro do container do Nyarch, é o loopback do próprio container e não a API do Windows — mesmo bloqueio documentado no README. Não copiar o endereço sem resolver o alcance. |
| `infra/n8n-local/workflows/Usami - Obsidian Inbox.json` e `Usami - Obsidian Inbox (local).json` | Padrão de **escrita de nota** com validação e um arquivo por evento (`timestamp-slug`); as duas variantes mostram os dois mounts (`/files/obsidian/usami/...` no Nyarch, `/home/node/vault/...` na stack local) | São webhooks de entrada; não produzem evento para a API. |
| `infra/n8n-local/workflows/Usami - Receber Respostas.json` | Padrão de **validação de contrato medido** (exige campo, aceita variação, idempotência por timestamp+slug) e o hábito de revalidar com payload real | Também só grava nota. |
| Instância (MCP): `Usami - Obsidian Inbox` `NnWm0PoOh4IBjtQS` e `Usami - Receber Respostas` `bJJ17mDS2I5r4De9` | Prova de que export versionado pode divergir do publicado; conferir antes de confiar | Não existe workflow de proatividade/lembrete na instância hoje. |

Regra que continua valendo: **o n8n não move o avatar e não fala**. Ele só
produz dado/nota; quem apresenta é o desktop.

### 3.2 O que precisaria ser criado (não criar agora)

1. **`Usami - Lembretes do Vault (agendado)`**
   - Gatilho: Schedule a cada 15 min (ou 5 min), `America/Sao_Paulo`.
   - Lê notas de `Inbox/Automations/` com frontmatter/convenção
     `lembrete: 2026-11-09T14:45:00-03:00`, `texto: ...`.
   - Valida (data ISO futura, texto ≤ 300 chars, sem campo solto) e grava
     **um arquivo consolidado determinístico**
     `Inbox/Automations/reminders.pending.json` (idempotente por
     `chave = data+slug`; rodar duas vezes no ciclo não duplica).
   - **Contrato de escrita atômica no SMB (n8n):** a gravação no SMB **não pode
     ser in-place** (risco de leitura de JSON incompleto/truncado pela API). O n8n
     deve salvar em `reminders.pending.json.tmp` e renomear atomicamente
     (`mv` / rename) para `reminders.pending.json`.
   - Caminho exato: `/home/node/vault/Inbox/Automations/reminders.pending.json`
     no container, que é o SMB `/mnt/usami-automations` já montado no compose do
     Nyarch (`nyarch/docker-compose.yml`, mount `target:
     /home/node/vault/Inbox/Automations`) e já coberto por
     `N8N_RESTRICT_FILE_ACCESS_TO`. A API local lê o mesmo arquivo em
     `<vault>/Inbox/Automations/reminders.pending.json`.
   - Não chama a API: isso contorna o bloqueio de alcance **sem abrir porta,
     sem mount novo e sem mudar o compose**. Dependência: o SMB precisa estar
     montado (`mountpoint /mnt/usami-automations`); desmontado, o container vê a
     pasta vazia.
   - **Gatilho de ingestão e tolerância na API:**
     - *O que dispara a ingestão:* (1) timer periódico local na API (a cada 60 s,
       no mesmo tick do scheduler / motor proativo), e (2) verificação sob demanda
       no `GET /api/proactive/events` comparando o `mtime` do arquivo com o carimbo
       da última ingestão.
     - *Tolerância a falha:* arquivo ausente (`ENOENT`), vazio (0 bytes), erro de I/O
       do SMB ou JSON malformado/incompleto são tratados silenciosamente como lista
       vazia `[]`, registrando apenas aviso (`console.warn`) sem crashar a API.
   - Na API, o default de vault hoje aponta para `Inbox/Responses`
     (`usami/src/rag.js` constante `DEFAULT_VAULT_RELATIVE`, linha ~16-17); o ingest
     precisa de caminho próprio (`USAMI_PROACTIVE_VAULT` ou constante) para
     `Inbox/Automations`. É uma mudança pequena e local, mas é mudança — não “já
     funciona”.
2. **`Usami - Resumo do Dia (agendado)`** — opcional.
   - Se o resumo tiver de incluir conteúdo do vault, um Schedule diário grava
     `Inbox/Automations/digest-AAAA-MM-DD.md` e a API o usa como fonte.
   - Se o resumo for só memória/histórico do chat, **não precisa de n8n**: a
     API compõe (recomendado, ver §4).
3. **`Usami - Saudação por horário`** — **não criar**. A API já tem as 8 faixas
   (`getTimeSlot`) e as cenas determinísticas; duplicar no n8n criaria duas
   fontes de saudação. Se o texto precisar morar no vault, é a API que lê.

Decisão pendente para o Lucas: esses workflows rodam na **stack local do
Windows** (a única que alcança a API) ou usam a **ponte do vault** na stack do
Nyarch (recomendada, zero porta nova)? Publicar no Nyarch sem a ponte produz
falha diária visível, como já aconteceu com o `Revisao de Tecnicas`.

---

## 4. Fronteira de responsabilidade

| Camada | Fica com | Não faz |
| --- | --- | --- |
| **n8n** (`infra/n8n-local/`) | Agendar tarefas de casa; transformar notas em arquivo de lembretes/resumo no vault; idempotência por data; falha visível (Stop and Error). | Não fala, não move avatar, não chama o desktop, não abre porta, não guarda segredo de usuário, não é fonte de saudação. |
| **API** (`usami/`) | Store persistente do inbox; relógio do motor proativo (`checkAndTrigger` em intervalo, hoje sem chamador); política (DND, frequência, afinidade, validade); dedupe por `dedupeKey`; rotas `events`/`ack`; ingestão do arquivo do vault; composição do resumo a partir da memória/histórico local; autenticação. | Não empurra para o desktop; não conhece UI; não manda evento para serviço externo; não gera texto por LLM por padrão. |
| **Desktop** (`usami-desktop/`) | Poll com backoff; seen-set persistido; fila de apresentação; balão + TTS via `/api/tts` já existente; cooldown; respeito a `is_busy`/chamada/sono; interruptor e horário silencioso na UI; status discreto de “proatividade”. | Não escuta porta, não abre socket de servidor, não fala com Postgres, não lê `.env` do projeto, não embarca token no zip, não lê/grava memória. |

### 4.1 O que muda na API (`usami/`) — depois de aprovação

- Estender `createProactiveHandler` (`server.mjs` rota `/api/proactive/`) com
  `GET events`, `POST events` e `POST events/ack`; o roteador passa a casar o
  prefixo `/api/proactive/`.
- Store novo + política de frequência/DND (reusar as constantes de
  `proactive-engine.js`).
- **Ligar o relógio e re-apontar o canal de eventos (A1):**
  chamar `proactiveEngine.start(60000)` ou scheduler próprio (hoje não há
  chamador).
  *Ponto crítico de canal:* o engine é criado em `server.mjs` (linha ~175) com
  `onProactiveSay: (cmd) => queueAvatarCommand(...)`, enviando para a fila
  volátil em memória at-most-once (`/api/n8n/commands`, podada em 120 s) que o
  desktop não consome. Apenas ligar o relógio jogaria `greeting`/`idle_nudge` no
  canal errado. O callback `onProactiveSay` **precisa ser re-apontado** para o
  store persistente novo (`proactive-events.json` / `/api/proactive/events`) com
  o envelope padrão (`type: 'greeting'`, `type: 'idle_nudge'`). Se a web precisar
  receber comandos de avatar, o callback pode fazer fanout, mas o store persistente
  é a fonte de verdade para o desktop.
- Ingestão resiliente de `reminders.pending.json` do vault (leitura local via
  timer a cada 60 s e sob demanda no `GET events` com verificação de `mtime`;
  ignora arquivo ausente ou corrompido).
- Resumo do dia a partir de `memory.json`/histórico (sem rede).
- Token: o desktop precisa do `USAMI_API_TOKEN`. Proposta: ler da **env do
  usuário** (mesmo padrão de chave em env já usado no projeto), nunca do zip.
  Sem token → recurso desligado e status explícito, não loop de 401.

### 4.2 O que muda no desktop — depois de aprovação

- Novo laço tokio (o app já usa `cx.spawn` + `background_executor().timer` no
  bloco de background tasks em `main.rs`) para poll/ack.
- Struct `serde` do evento + `seen-set` em `%APPDATA%\Usami\`.
- Reuso do caminho de fala de `submit(&mut self, cx: &mut Context<Self>)` em
  `main.rs` para balão + `stream_tts`, **sem** passar por `/api/chat`.
- Dois controles em CONVERSA/SOBRE: liga/desliga proatividade e horário
  silencioso; persistidos localmente.
- Nenhuma dependência nova obrigatória (reqwest/tokio/serde já estão lá).

### 4.3 O que **não** muda

- Fila `/api/n8n/*` e comportamento da web.
- Workflows publicados, compose, volumes, mounts.
- Regra de que o desktop não lê memória nem personalidade.
- Nenhuma porta, túnel ou bind não-loopback.

---

## 5. Privacidade e ruído

### 5.1 Nenhum dado para serviço externo

- Eventos nascem e morrem na máquina: n8n grava nota no vault (SMB já
  existente), API lê local, desktop exibe local. Nenhum POST para fora.
- **Saudação e resumo v1 são determinísticos** (`scenes.js`/`scenes-data.js`);
  o engine já é criado sem router (`server.mjs:175`), então não há chamada a
  NVIDIA/OpenRouter. Se um dia alguém ligar router no motor, o “contexto do
  dia” (que descreve a rotina do usuário) passaria a ir para o provedor — isso
  exige aprovação explícita e não faz parte deste contrato.
- **Exceção a declarar na UI:** `speak:true` manda o texto para o `/api/tts`,
  que usa o provedor de voz configurado. Para lembrete sensível, o produtor
  marca `speak:false` ou o usuário liga “só texto”. O contrato nunca manda o
  texto para lugar nenhum além do backend local que o usuário já usa.
- Log: registrar `id`, `type`, `outcome` e latência; **nunca** o `text`.
- Retenção: evento removido no ack/expiração; seen-set limitado a 200 ids; sem
  backup extra, sem replicação.

### 5.2 Frequência máxima (aplicada no servidor, reforçada no desktop)

| Tipo | Teto |
| --- | --- |
| `greeting` | 1 por faixa de horário/dia; máx. 4/dia |
| `idle_nudge` | 1 a cada 60 min; máx. 3/dia; só com idle ≥ 3 min e afinidade ≥ 20 (regra atual) |
| `daily_digest` | 1/dia |
| Social combinado | máx. 6 eventos sociais/dia somando `greeting` + `idle_nudge` + `daily_digest` |
| `reminder` | máx. 3/hora e 6/dia; cada ocorrência uma vez; **cota isolada** |
| Global de segurança | máx. 12 notificações/dia somando todos os tipos; mínimo 5 min entre falas de `greeting`/`idle_nudge` |

**Precedência e isolamento de cotas (M1):**
A soma dos máximos nominais (4 + 3 + 1 + 6 = 14) não pode permitir que saudações
ou nudges esgotem a cota e façam um lembrete do usuário receber 429.
1. `reminder` **não consome o teto social** e tem reserva mínima garantida de até
   6/dia. Lembrete nunca recebe 429 nem é suprimido por causa de saudações
   anteriores.
2. Os eventos gerados espontaneamente pelo motor (`greeting`, `idle_nudge`,
   `daily_digest`) compartilham o teto social de até 6/dia; ao atingir o teto,
   novas saudações/nudges são simplesmente não agendados.
3. Excesso no teto de `reminder` (mais de 3/h ou 6/dia) devolve 429 com
   `retryAfter` ao produtor; nada é enfileirado “para depois” além do teto.

Janela padrão por tipo (evita saudação/resumo velho ao abrir o app à noite):

| Tipo | `notBefore` | `expiresAt` |
| --- | --- | --- |
| `greeting` | início da faixa (`05/08/12/14/17/19/22/01`) | fim da faixa, no máximo 60 min |
| `idle_nudge` | após 3 min de inatividade reportada | +15 min |
| `daily_digest` | 21:00 | 23:59 do mesmo dia |
| `reminder` | horário do lembrete | +12 h, no máximo |

### 5.3 Horário silencioso

- Padrão **23:30–08:30** (o mesmo do engine), configurável no desktop;
  comparação no relógio local do usuário, eventos em ISO com offset
  `-03:00` (`America/Sao_Paulo`).
- Durante o silêncio: `greeting`/`idle_nudge` são confirmados como
  `suppressed` (não voltam); `reminder`/`daily_digest` ficam retidos sem ack e
  aparecem quando o silêncio termina, se ainda dentro da validade.
- `priority: high` **não** quebra o silêncio com som por padrão; no máximo
  mostra o balão sem TTS. Quebrar o silêncio é opt-in do usuário.
- Modo companhia (sono de 30 min, `main.rs` campo `sleep_companion_started_at`
  no laço do timer) e chamada em andamento também suprimem.

### 5.4 Como desligar

1. **Desktop:** interruptor “Proatividade” (persistido). Desligado → o laço de
   poll para, a fila local é limpa e nada é exibido; eventos no servidor
   expiram sozinhos.
2. **API:** kill switch de ambiente (`USAMI_PROACTIVE_ENABLED=0`, previsto na
   implementação) — o motor não agenda e as rotas devolvem lista vazia; depois
   de implementado, desligar não exige mexer em código.
3. **Por tipo:** `reminder` e `daily_digest` podem ser desligados
   separadamente sem matar a saudação.
4. Desligar **não** apaga evento já confirmado nem toca no vault.

---

## 6. Riscos e decisões que são do Lucas

| # | Decisão | Recomendação |
| --- | --- | --- |
| 1 | Quem produz lembrete: scheduler da API ou workflow n8n | API agenda; n8n só escreve o arquivo no vault (ponte) |
| 2 | Token do desktop: env do usuário vs rota loopback sem token | Env (`USAMI_API_TOKEN`); rota sem token contraria o fail-closed atual |
| 3 | `high` pode quebrar o silêncio | Não por padrão (só balão) |
| 4 | Duas instâncias do desktop na mesma máquina | v1 tem inbox único; as duas podem exibir o mesmo evento na janela de poll (corrida aceita, sem fala repetida depois do ack) |
| 5 | Resumo do dia com LLM para variar o texto | Não na v1; determinístico, para não mandar rotina a provedor |
| 6 | Web e desktop compartilham proatividade | Não na v1: web segue na fila `proactive_say`; desktop usa o inbox novo |
| 7 | Janela oculta na bandeja: reter evento ou notificar pela bandeja | v1 retém até a janela aparecer; notificação de bandeja é evolução |

Riscos residuais: SMB do vault fora do ar (lembrete não nasce; falha visível no
n8n), relógio/fuso divergente entre máquinas (usar sempre ISO com offset),
store JSON crescendo sem poda (teto de 200), e o fato de a API **ainda não
persistir** a fila de comandos atual — por isso o inbox novo nasce com store
próprio, não reaproveita `n8n-queue.js`.

---

## 7. Verificação prevista (depois da aprovação, com payload real)

1. Criar uma nota de lembrete real no vault, rodar o workflow e capturar o
   `reminders.pending.json` gerado; ingerir na API e conferir o evento com o
   `dedupeKey` real (não sintético).
2. Rodar o mesmo ciclo duas vezes e provar que o `dedupeKey` atualiza em vez de
   duplicar (contagem = 1).
3. Fechar o desktop após o GET e antes do ack; reabrir depois de 5 min e provar
   a redelivery + supressão pelo seen-set (fala uma vez, ack reenviado).
4. Reiniciar a API entre `due` e o poll e provar que o evento persistido chega.
5. Forçar `notBefore` no futuro e `expiresAt` no passado; provar retenção e
   descarte.
6. Simular horário silencioso (clock controlado) e provar `suppressed` para
   saudação e retenção para lembrete.
7. Sem token: provar status “proatividade indisponível” e ausência de loop de
   401 (backoff até 60 s).
8. Confirmar que nenhum evento dispara enquanto `is_busy`, TTS ou chamada.

Até lá, **nada foi alterado**: workflows, compose, volumes, containers, vault e
credenciais seguem como estavam.
