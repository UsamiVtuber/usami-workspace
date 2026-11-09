# API local para as abas de configuração do desktop

**Status:** investigação read-only, concluída em 2026-11-08. Nada foi implementado.
**Repositórios:** `usami/` no HEAD `95fe4e3` (working tree limpa); `usami-desktop/`
no HEAD `31ebde8` com WIP local de presença (`src/presence.rs`, `Cargo.toml`,
`Cargo.lock`, `src/main.rs`) preservado e não tocado.
**Backend exercitado:** `npm run api` dentro de `usami/`, escutando em
`127.0.0.1:8787`. `USAMI_API_TOKEN` está configurado (o valor não foi lido nem
impresso em nenhum momento).
**Regras cumpridas:** nenhum arquivo em `usami/` foi alterado; `vault/` e
`assets/` não foram tocados; não houve commit nem push. Todos os exemplos deste
documento usam dados sintéticos — o conteúdo real de `memory.json` e
`personality.json` foi omitido.

---

## 0. Resumo executivo

1. **MEMÓRIA** e **JEITO** são as duas únicas abas com backend pronto. Leitura e
   exclusão de memória e leitura/gravação de personalidade existem e foram
   exercitadas com payload real.
2. **O bloqueio atual do desktop não é de contrato, é de autenticação:** memória,
   personalidade e STT exigem `x-usami-token`; o desktop não envia o header.
   Com o token configurado no servidor, essas chamadas retornam **401** hoje
   (validado). Chat e TTS não exigem token e continuam funcionando.
3. **Memória editável de verdade não existe no backend.** Há `GET /api/memory` e
   `POST /api/memory/forget` (exclusão granular). Não existe rota para criar ou
   alterar um fato, editar o nome/apelido do usuário ou ajustar a afinidade;
   `POST /api/memory` responde **404** (validado).
4. **CONVERSA, VOZ, AVATAR e SOBRE não têm endpoints de configuração/status.**
   `/api/health`, `/api/status`, `/api/config`, `/api/models`, `/api/voice` e
   similares respondem **404**. O que essas abas mostram hoje é texto estático ou
   dado local do próprio desktop.
5. **Duas decisões são do Lucas antes de qualquer implementação:** como o
   desktop obtém o token (sem embarcar segredo no executável) e se a edição de
   memória ganha uma rota nova de backend (não existe hoje).

---

## 1. Como a validação foi feita

Duas frentes, ambas contra o código real:

- **Instância real** (`127.0.0.1:8787`): GETs e sondas de rotas inexistentes.
  Nenhuma escrita foi disparada contra a memória/personalidade reais.
- **Instância isolada**: os mesmos handlers de `usami/server.mjs` importados por
  uma sonda descartável fora dos repositórios, com `USAMI_DATA_DIR` apontando
  para uma pasta temporária. Foi nela que os POSTs destrutivos (memória),
  gravações de personalidade, TTS e o chat real rodaram. A semente de memória foi
  gravada **antes** do import porque o store do handler carrega o arquivo uma vez
  só, na construção (`usami/server.mjs:162-171`).

| Sonda | Resultado |
| --- | --- |
| `GET /api/memory` sem token (instância real) | **401** `{"error":"Token ausente ou inválido."}` |
| `GET /api/memory` com token (instância real) | **200**, shape confirmado; conteúdo omitido |
| `GET /api/personality` sem token (instância real) | **401** |
| `GET /api/personality` com token (instância real) | **200**, shape confirmado; conteúdo omitido |
| `POST /api/stt` sem token / token errado (instância real) | **401** / **401** |
| `POST /api/stt` com token, WAV de 0,5 s de silêncio | **200** `{"text":""}` em ~15 s (whisper frio) |
| `POST /api/tts` sem token (instância real) | **200** `audio/wav`, 90.598 bytes, `RIFF` |
| `POST /api/memory` (instância real) | **404** |
| `PUT /api/memory/forget` (instância real) | **404** |
| `/api/health`, `/api/status`, `/api/config`, `/api/voice`, `/api/avatar`, `/api/model`, `/api/models`, `/api/settings`, `/api/version`, `/api/engine`, `/api/tts/config`, `/api/chat/config`, `/api/memory/config` | **404** em todas |
| `POST /api/memory/forget` sem `confirm` (isolada) | **403** |
| `POST /api/memory/forget` com `confirm:"APAGAR"` (isolada) | **200**, `removed: 1` |
| `POST /api/memory/forget` `kind` inválido / seção inválida / data inválida | **400** / **400** / **400** |
| `POST /api/personality` com valores fora da faixa (isolada) | **200**, normalizado |
| `POST /api/chat` (isolada, mensagem sintética) | **200** em ~6,1 s, `source:"nvidia"`, com `timing` |
| `POST /api/tts` (isolada, texto sintético) | **200** `audio/wav`, 80.998 bytes, `RIFF` |

---

## 2. Inventário de endpoints reais

Toda rota `/api` passa por `isAllowedRequest` (Host/Origin) em
`usami/server.mjs:2400-2415` e `usami/src/api-security.js:44-60`. Requisição
nativa sem `Origin` é aceita — é por isso que o desktop conversa hoje.

### 2.1 `GET /api/memory`

- **Auth:** `x-usami-token` obrigatório (`server.mjs:1163`, `api-security.js:70-77`).
  Sem token no servidor → **503** (fail-closed); token ausente/errado → **401**.
- **Request:** sem corpo.
- **Response 200:** snapshot para a UI, **sem o texto das mensagens do histórico**
  (o histórico volta só como contagem por dia) — `server.mjs:1077-1091`.
- **Evidência:** handler em `usami/server.mjs:1147-1167`; rota em
  `usami/server.mjs:2409`.

```json
{
  "ok": true,
  "memory": {
    "version": 2,
    "user": { "name": null, "nickname": null, "createdAt": "2026-11-09T02:53:21.338Z" },
    "sections": {
      "user": [],
      "preferences": ["gosto por chá gelado"],
      "hobbies": [],
      "relationship": [],
      "dates": [],
      "other": []
    },
    "recentFacts": [
      { "fact": "prefere manhãs tranquilas", "section": "other",
        "date": "2026-11-09", "expiresAt": "2026-11-16", "expired": false }
    ],
    "summaries": [
      { "date": "2026-11-01", "summary": "Conversa sintética sobre rotina." }
    ],
    "history": [ { "date": "2026-11-09", "count": 2 } ],
    "affinity": { "score": 42, "lastDailyGainDate": "2026-11-09", "totalDailyGainToday": 0 }
  }
}
```

Observações de contrato:

- `sections` é sempre um objeto com as seis chaves fixas
  (`src/memory-store.js:25`).
- Fatos são **strings**, sem id. A exclusão é por texto exato (maiúsculas/minúsculas
  ignoradas), não por índice (`src/memory-store.js:361-373`).
- `recentFacts[].expired` é calculado no momento do GET (`server.mjs:1083`).
- `history` é `[{date,count}]` sem conteúdo — não há como ler as mensagens
  guardadas pelo servidor por esta API.
- A afinidade do servidor (`affinity.score`, 0–100) é **diferente** dos medidores
  de intimidade do web, que vivem em `localStorage` (`src/bond.js`,
  `src/Usami.jsx:89`).

### 2.2 `POST /api/memory/forget`

- **Auth:** token obrigatório (`server.mjs:1163`).
- **Content-Type:** `application/json` (senão **403**) — `server.mjs:1173-1176`.
- **Limite de corpo:** 8 KiB (`server.mjs:1178`).
- **Confirmação:** `confirm` precisa ser `APAGAR` após `trim` + `toUpperCase`;
  `"apagar"` é aceito (`server.mjs:1093,1193-1195`).
- **Kinds** (`server.mjs:1094,1198-1234`):

| `kind` | Campos | Efeito |
| --- | --- | --- |
| `fact` | `section`, `fact` | remove um fato durável da seção (match exato, case-insensitive) |
| `recent` | `fact` | remove um fato recente pelo texto |
| `section` | `section` | esvazia a seção inteira |
| `summary` | `date` (`YYYY-MM-DD`) | remove o resumo do dia |
| `history` | `date` opcional (`YYYY-MM-DD`) | zera o histórico daquele dia; sem `date`, zera todos os dias |
| `all` | — | `store.reset()`: apaga tudo e recria o padrão |

- **Response 200:** `{ok, forgotten, removed, memory}` com o snapshot atualizado
  (`server.mjs:1236`). Atenção: em `kind:"all"` o campo `removed` é **sempre 0**
  (não é calculado), então “0” não significa “nada foi apagado”.
- **Erros:** `kind` inválido → 400; seção inválida → 400; fato ausente → 400;
  data fora de `YYYY-MM-DD` → 400; sem confirmação → 403.
- **Evidência:** `usami/server.mjs:1147-1238`, rota em `usami/server.mjs:2409`.

### 2.3 `GET /api/personality`

- **Auth:** token obrigatório (`server.mjs:1115`; rota em `server.mjs:2410`).
- **Response 200:** `{ok, personality, traits, customPromptMax}`
  (`server.mjs:1118-1120`).
- `personality` = `{version:1, traits:{warmth,curiosity,playfulness,pride}, customPrompt}`.
- `traits` é o catálogo de rótulos vindo de `TRAITS`
  (`src/personality-store.js:18-39`); `customPromptMax` é **600**
  (`src/personality-store.js:16`).

```json
{
  "ok": true,
  "personality": {
    "version": 1,
    "traits": { "warmth": 50, "curiosity": 50, "playfulness": 50, "pride": 50 },
    "customPrompt": ""
  },
  "traits": [
    { "key": "warmth", "label": "Carinho" },
    { "key": "curiosity", "label": "Curiosidade" },
    { "key": "playfulness", "label": "Brincadeira" },
    { "key": "pride", "label": "Orgulho (tsundere)" }
  ],
  "customPromptMax": 600
}
```

### 2.4 `POST /api/personality`

- **Auth:** token obrigatório (`server.mjs:1115`).
- **Content-Type:** `application/json` (senão 403); corpo ≤ 4 KiB
  (`server.mjs:1123-1127`).
- **Request:** o objeto completo `{traits:{...}, customPrompt:"..."}` ou
  `{reset:true}` (`server.mjs:1138-1139`). Não é PATCH: traço ausente cai no
  neutro 50; chave desconhecida é ignorada; valor é arredondado e preso a 0–100;
  `customPrompt` é sanitizado (caracteres de controle removidos, CRLF normalizado)
  e cortado em 600 (`src/personality-store.js:60-90`).
- **Response 200:** `{ok, personality}` **sem** o catálogo `traits` nem
  `customPromptMax` — a UI deve manter o catálogo obtido no GET.
- **Evidência:** `usami/server.mjs:1100-1145`.

### 2.5 `POST /api/chat` (fonte de status da aba CONVERSA)

- **Auth:** **não exige token**; só Host/Origin (`server.mjs:488-496`).
- **Request:** JSON, corpo ≤ 8 KiB, `message` string não vazia ≤ 2000
  (`server.mjs:316-327,503-506`). Campos opcionais lidos no topo:
  `capabilities`, `wakeUpJolt`, `sleepCompanionActive`.
- **Response 200:** `server.mjs:827-837`.

```json
{
  "reply": "Ah... oi. Você de novo?",
  "gesture": "tilt",
  "actions": [ { "type": "set_expression", "name": "happy", "weight": 0.3 } ],
  "source": "nvidia",
  "surface": { "status": "ok", "tone": "casual_happy", "mode": "say",
               "gesture": "wave", "reply": "Oi! Tudo bem?" },
  "timing": { "surfaceMs": 0, "llmMs": 6057, "animationMs": 2,
              "contextMs": 84, "totalMs": 6153 },
  "affinity": 0,
  "stage": "curiosidade_distante"
}
```

- `speechTags` só aparece quando existe (`server.mjs:830`).
- `affinity`/`stage` só aparecem quando a gravação da memória deu certo
  (`server.mjs:756-785,836`).
- `reply` é limitado a ~220 caracteres (`src/provider-parse.js:20`);
  `actions` passam pelo filtro do catálogo (`server.mjs:803`).
- **Efeitos colaterais:** o turno grava histórico e afinidade
  (`server.mjs:756-788`), dispara resumo de dias pendentes em background
  (`server.mjs:793-801`) e publica evento no webhook do n8n
  (`server.mjs:808-817`). Um “testar conexão” com `/api/chat` **não é gratuito
  nem inofensivo**.
- O `ChatResponse` do desktop declara só `reply/gesture/actions/source/speechTags`
  (`usami-desktop/src/api.rs:70-78`): `timing`, `affinity`, `stage` e `surface`
  chegam e são descartados hoje; `source` é desserializado mas não usado.

### 2.6 `POST /api/tts` (VOZ)

- **Auth:** **não exige token**; só Host/Origin (`server.mjs:1503-1507`).
- **Request:** JSON ≤ 8 KiB (`server.mjs:329-343`):
  `{text ≤2000, styleId? (≤80, padrão "neutral"), rate? (padrão 1),
  pitch? (padrão 1), stream?: true}`.
- **Response:** áudio, nunca JSON de configuração.
  - Gemini em `stream:true` → `audio/l16; codec=pcm; rate=24000` + `X-TTS-Stream: 1`
    com PCM Int16 LE cru (`server.mjs:1402-1407`).
  - Caminho normal → `audio/wav` (Gemini) ou `audio/mpeg` (ElevenLabs)
    (`server.mjs:1544,1695`), com `X-TTS-Cache: HIT` quando veio do cache
    (`server.mjs:1562-1568`).
- **Semáforo:** uma síntese por vez, fila 4; cheio → 429, esgotado → 503
  (`server.mjs:1574-1587`).
- **Não há endpoint para ler voz, modelo, instrução de estilo ou provedor.**
  A resposta não ecoa nenhum desses valores.

### 2.7 `POST /api/stt` (VOZ em chamada)

- **Auth:** token obrigatório (`server.mjs:1809`).
- **Request:** multipart, campo `audio` (WAV), teto de 8 MiB
  (`server.mjs:1783-1791`).
- **Response 200:** `{"text":"..."}` (`server.mjs:1870`) — vazio para silêncio.
- **Observação crítica:** o desktop chama `/api/stt` sem header de token
  (`usami-desktop/src/api.rs:199-228`), então com `USAMI_API_TOKEN` configurado
  a chamada por voz recebe **401** hoje.

### 2.8 Rotas que existem mas não servem as abas

| Rota | Para que serve | Por que não serve |
| --- | --- | --- |
| `GET /api/proactive/state` | estado do motor proativo: `{lastUserActivityMs, unansweredCount, isBusy, idleMs}` (`src/proactive-engine.js:206-213`) | não é status de engine/modelo; exige token; o motor está dormente (sem `start()`) |
| `GET/POST /api/n8n/commands*` | fila de comandos do n8n | é fila por cliente, não configuração |
| `GET /api/rag/context` | contexto do vault | exige token; não é memória editável |
| `GET/POST /api/technique[s]`, `/api/plan`, `/api/learning*` | técnicas, planner e aprendizagem | não alimentam nenhuma das seis abas |
| `POST /api/media/analyze` | análise de imagem/vídeo | não alimenta nenhuma das seis abas |

### 2.9 Rotas que **não existem** (404 confirmado)

`/api/health`, `/api/status`, `/api/config`, `/api/voice`, `/api/avatar`,
`/api/model`, `/api/models`, `/api/settings`, `/api/version`, `/api/engine`,
`/api/tts/config`, `/api/chat/config`, `/api/memory/config`.

Também não existe:

- `POST /api/memory` (criar/alterar fato) → **404**;
- qualquer método além de `GET` em `/api/memory` e `POST` em `/api/memory/forget`;
- rota para editar `user.name`/`user.nickname` ou a afinidade.

Se a intenção for editar memória, hoje **não há contrato** para isso. Qualquer
rota nova fica fora desta investigação.

---

## 3. O que cada aba consegue fazer

| Aba | Com a API atual | Falta de backend | Só UI |
| --- | --- | --- | --- |
| CONVERSA | mostrar backend local + `source`/`timing` da última resposta | status de engine/provedor/modelo; endpoint de saúde/config | histórico, exportar/limpar, medidores, textos |
| VOZ | nada de configuração; TTS/STT já usados pelo app | leitura de voz/modelo/instrução/provedor | descrição, chips de tags, privacidade |
| AVATAR | nada | nada (se o avatar for local); não há estado compartilhado com o web | fundo, blur, tela cheia, inspeção de rig, escolha de modelo |
| MEMÓRIA | ler snapshot e excluir (token) | criar/editar fato, editar nome/apelido, ajustar afinidade | lista, gate `APAGAR`, confirmações |
| JEITO | ler e gravar personalidade (token) | nada para os 4 traços atuais | sliders, contador 600, salvar/restaurar |
| SOBRE | nada | versão do backend (não exposta) | versão, tecnologias, créditos, privacidade |

### CONVERSA

- **Hoje no desktop:** `usami-desktop/src/main.rs:2104` (aba) e `2166-2206`
  (conteúdo). Renderizador e modelo são texto fixo; só o backend é dinâmico
  (`self.api_client.base_url()`).
- **Com a API atual:** o `POST /api/chat` devolve `source`, `timing`,
  `affinity`, `stage` e `surface` (seção 2.5). Dá para mostrar “última resposta
  por: NVIDIA/OpenRouter/offline”, latência e estágio de afinidade — mas o
  desktop precisa desserializar esses campos e guardar o último turno.
- **Falta de backend:** não há como perguntar “qual modelo está carregado?”,
  “quais chaves existem?” ou “o motor está saudável?”. Não existe `/api/health`
  nem `/api/config`. `/api/proactive/state` não responde isso.
- **Só UI:** lista de histórico (o desktop mantém `chat_history` em memória,
  `main.rs:348`), limpar/exportar, medidores de vínculo (hoje incrementados
  localmente em `main.rs:762-767`), estados vazios e textos.
- **Cuidado:** o conteúdo da aba CONVERSA do web é `localStorage`
  (`src/conversation-storage.js:6`, `src/Usami.jsx:2401-2419`), não API. O
  “status do motor” do desktop é uma invenção local, não paridade.

### VOZ

- **Hoje no desktop:** `main.rs:2104` e `2207-2248`, todo estático.
- **Com a API atual:** nada de leitura. `POST /api/tts` só devolve áudio; não
  informa voz, modelo, provedor ou instrução. `POST /api/stt` só devolve texto.
- **Falta de backend:** não existe GET de configuração de voz. Trocar voz/modelo
  continua exigindo editar `.env` e reiniciar a API — o texto do desktop já diz
  isso (`main.rs:2229-2231`).
- **Só UI:** descrição da voz oficial, chips `<laugh>/<chuckle>/<sigh>/<short pause>`,
  aviso de privacidade, nomes de variáveis. Um botão “testar voz” sintetizaria de
  verdade (custo e latência), não seria um health check.
- **Bug de contrato já existente:** o chat devolve `speechTags` (ex.:
  `["short pause"]`), mas o desktop envia `reply` cru ao TTS e ignora as tags
  (`main.rs:798`). É ajuste de cliente, não de backend.
- **Bloqueio de chamada:** `/api/stt` exige token (2.7) e o desktop não envia.

### AVATAR

- **Hoje no desktop:** `main.rs:2104` e `2249-2334`.
- **Com a API atual:** nada — não há `/api/avatar`, `/api/model` ou `/api/rig`.
- **Falta de backend:** nenhuma para um avatar nativo. O web também não persiste
  nada no servidor: fundo, blur, arquivo e rig são locais do navegador
  (`src/Usami.jsx:2428-2490`).
- **Só UI/local:** `USAMI_VRM_PATH`, `USAMI_VRMA_DIR` e `USAMI_BACKGROUND`
  (`README.md:134-145`), câmera/blur/tela cheia, escolha de arquivo e a inspeção
  do modelo pelo `gpui-vrm` (formato, meshes, ossos, animações, expressões,
  morphs).
- **Se houver paridade de estado salvo:** o desktop precisaria do próprio arquivo
  de configuração local (não existe) — decisão de produto, não endpoint.

### MEMÓRIA

- **Hoje no desktop:** `main.rs:2104` e `2335-2372`, com contadores fixos em
  “0 fato(s)…” e o aviso de que o painel não lê nem apaga.
- **Com a API atual (bloqueado só pelo token):** contadores por seção, recentes,
  resumos, dias de histórico e afinidade; listar fatos; apagar fato/recente/seção/
  resumo/dia/tudo com o gate `APAGAR`.
- **Falta de backend:** criar e editar fato; editar `user.name`/`nickname`;
  mudar afinidade; desfazer; ler o texto das mensagens do histórico (só há
  contagem). Ver seção 4.
- **Só UI:** agrupar por seção, campo `APAGAR`, confirmação por item, estados de
  carregamento/erro, atualização do snapshot após cada escrita.
- **Referência web:** `src/Usami.jsx:691-733` (load/forget) e a renderização logo
  abaixo; `src/PersonalityPanel.jsx` para o padrão de dirty/save.

### JEITO

- **Hoje no desktop:** `main.rs:2104` e `2373-2424`, sliders e textarea
  desabilitados com “0/600”.
- **Com a API atual (bloqueado só pelo token):** GET para carregar os quatro
  traços + `customPrompt`; POST para salvar o objeto completo ou `{reset:true}`.
  É a aba com implementação mais direta.
- **Falta de backend:** nada para os quatro traços atuais. Perfis por usuário
  estão fora do runtime de um usuário só (`usami/AGENTS.md:54`). Não há PATCH
  parcial nem controle de concorrência: o último POST vence.
- **Só UI:** sliders 0–100, contador 600, estado “sujo”, salvar, restaurar,
  aviso e erro. O POST não devolve o catálogo; manter o do GET.

### SOBRE

- **Hoje no desktop:** `main.rs:2425-…`, versão 0.2.0 e créditos estáticos.
- **Com a API atual:** nada.
- **Falta de backend:** não há `/api/version`; a versão do backend não é exposta.
- **Só UI:** versão do `Cargo.toml` (0.2.0), tecnologias, créditos
  (`docs/CREDITS.md`) e o texto de privacidade.

---

## 4. Painel de memória editável: o que existe de fato

### 4.1 Dá para fazer hoje (somente leitura + exclusão)

- Ler o snapshot completo (2.1) e resumir: fatos por seção, recentes, resumos,
  dias com histórico e afinidade. O web já faz exatamente isso
  (`src/Usami.jsx:751-756`).
- Excluir com confirmação escrita:
  - um fato durável (`kind:"fact"`);
  - um fato recente (`kind:"recent"`);
  - uma seção inteira (`kind:"section"`);
  - o resumo de um dia (`kind:"summary"`);
  - o histórico de um dia ou de todos os dias (`kind:"history"`);
  - tudo (`kind:"all"`).
- O servidor devolve o snapshot atualizado na mesma resposta, então a UI pode
  substituir o estado sem um GET extra.

### 4.2 Não existe hoje

- **Adicionar** um fato manualmente.
- **Editar** o texto de um fato (só apagar e esperar o chat aprender de novo).
- **Editar** `user.name` / `user.nickname` (o snapshot expõe, mas não há escrita).
- **Ajustar** `affinity.score` sem apagar tudo.
- **Desfazer** qualquer exclusão.
- **Ler** o conteúdo das mensagens do histórico do servidor — só a contagem por
  dia é exposta (`server.mjs:1085-1088`).

Conclusão: “painel de memória editável” só é possível hoje no sentido de
**inspecionar e excluir**. Edição de conteúdo exige uma decisão de backend que
não existe nesta API.

### 4.3 Semântica de exclusão que a UI precisa respeitar

- O match de `fact`/`recent` é por **texto exato, trim + case-insensitive**
  (`src/memory-store.js:361-373,385-394`); não há id, então a UI deve usar o
  próprio texto como chave e lidar com duplicatas improváveis.
- `removed` é a contagem real em `fact`, `recent`, `section`, `summary` e
  `history`; em `all` é sempre 0 (`server.mjs:1230-1236`).
- `history` sem `date` apaga **todos** os dias; com `date`, apaga só aquele dia
  (`server.mjs:1220-1228`). A data do snapshot é a do fuso do servidor
  (`src/memory-store.js:405-431`).
- `all` não é “apagar fatos”: `store.reset()` recria o padrão, incluindo
  `user.createdAt`, afinidade e histórico (`src/memory-store.js:448-451`).
- Exclusão é imediata e sem desfazer. A única “confirmação” é o token digitado
  no corpo da requisição.

---

## 5. Contrato validado com payload real (exemplos redigidos)

### 5.1 Memória

Request de exclusão sem confirmação:

```http
POST /api/memory/forget
Content-Type: application/json
x-usami-token: <token do servidor>

{"kind":"fact","section":"preferences","fact":"gosto por chá gelado"}
```

Response real:

```json
{ "error": "Confirmação escrita necessária: envie confirm: \"APAGAR\"." }
```

→ **403** (validado na instância isolada).

Mesmo request com confirmação (fato sintético existente):

```json
{"kind":"fact","section":"preferences","fact":"gosto por chá gelado","confirm":"APAGAR"}
```

```json
{
  "ok": true,
  "forgotten": "fact",
  "removed": 1,
  "memory": { "version": 2, "sections": { "preferences": [] } }
}
```

→ **200** (o exemplo acima omite o restante do snapshot; o corpo real traz tudo).

Validações observadas:

| Corpo | Status | Erro |
| --- | --- | --- |
| `{"kind":"edit","confirm":"APAGAR"}` | 400 | `Tipo inválido. Use fact, recent, section, summary, history ou all.` |
| `{"kind":"section","section":"inventada","confirm":"APAGAR"}` | 400 | `Seção inválida.` |
| `{"kind":"summary","date":"09/11/2026","confirm":"APAGAR"}` | 400 | `Data inválida (YYYY-MM-DD).` |
| `{"kind":"fact", ...}` sem `confirm` | 403 | confirmação escrita |

### 5.2 Personalidade

Request sintético com valores fora da faixa:

```json
{
  "traits": { "warmth": 150, "curiosity": -5, "playfulness": "abc", "pride": 50, "inventado": 99 },
  "customPrompt": "<700 caracteres>"
}
```

Response real:

```json
{
  "ok": true,
  "personality": {
    "version": 1,
    "traits": { "warmth": 100, "curiosity": 0, "playfulness": 50, "pride": 50 },
    "customPrompt": "<600 caracteres>"
  }
}
```

→ **200**. Confirma: clamp 0–100, arredondamento, traço inválido → 50, chave
desconhecida descartada, prompt cortado em 600. `{reset:true}` devolve os quatro
traços em 50 e prompt vazio.

### 5.3 Chat

Request sintético:

```json
{
  "message": "Oi! Como você está hoje? (mensagem sintética de validação)",
  "capabilities": { "format": "VRM", "gestures": ["tilt", "wave"],
                    "expressions": ["happy"], "sequences": true },
  "sleepCompanionActive": false
}
```

Response real (instância isolada, provedor respondeu):

```json
{
  "reply": "Ah... oi. Você de novo?",
  "gesture": "tilt",
  "actions": [ { "type": "set_expression", "name": "happy", "weight": 0.3 } ],
  "source": "nvidia",
  "surface": { "status": "ok", "tone": "casual_happy", "mode": "say",
               "gesture": "wave", "reply": "Oi! Tudo bem?" },
  "timing": { "surfaceMs": 0, "llmMs": 6057, "animationMs": 2,
              "contextMs": 84, "totalMs": 6153 },
  "affinity": 0,
  "stage": "curiosidade_distante"
}
```

→ **200** em ~6,1 s. `reply`/`actions`/`source`/`timing`/`affinity`/`stage`
confirmados. `speechTags` aparece só quando o provedor devolve tag.

### 5.4 TTS

Request sintético:

```json
{ "text": "Oi, tudo bem?", "styleId": "neutral", "stream": false }
```

Response real (sem token):

```http
HTTP/1.1 200 OK
content-type: audio/wav
cache-control: no-store
x-content-type-options: nosniff
```

Corpo: 90.598 bytes começando com `RIFF`. Nenhum campo de voz/modelo/provedor no
corpo ou nos headers. Em `stream:true` com Gemini, os headers mudam para
`audio/l16; codec=pcm; rate=24000` + `X-TTS-Stream: 1` e o corpo é PCM cru.

### 5.5 STT

```http
POST /api/stt
Content-Type: multipart/form-data; boundary=...
x-usami-token: <token do servidor>
```

Com WAV de 0,5 s de silêncio → **200** `{"text":""}` em ~15 s. Sem token ou com
token errado → **401** `{"error":"Token ausente ou inválido."}`. O desktop hoje
não envia o header.

### 5.6 Observação de método

A instância isolada usou o mesmo `server.mjs` com `USAMI_DATA_DIR` temporário e
semeou o arquivo **antes** do import — o store do handler carrega o disco uma vez
na construção (`server.mjs:162-171`). Semear depois do import faz o handler
trabalhar com o snapshot antigo e foi descartado na primeira tentativa. Isso é
relevante para testes futuros: qualquer teste que injete memória precisa semear
antes de importar o módulo ou injetar o store no handler.

---

## 6. Riscos

### 6.1 Autenticação e segredo

- **Token é o bloqueio atual.** Com `USAMI_API_TOKEN` configurado, memória,
  personalidade, STT, RAG e fila do n8n respondem 401 para o desktop. Sem token
  no servidor, respondem 503 (fail-closed). Chat e TTS não são tokenizados.
- **Como o desktop obtém o token é uma decisão em aberto.** O executável não pode
  embarcar o segredo (o pacote é público). Opções como variável de ambiente,
  arquivo local com ACL ou leitura do `.env` têm consequências de segurança
  distintas e não foram decididas.
- **Host allowlist:** o desktop aceita `USAMI_API_BASE`; apontar para um host ou
  porta fora da allowlist (`127.0.0.1:8787`/`localhost:8787` +
  `USAMI_ALLOWED_HOSTS`) produz **403**, mesmo com token correto
  (`src/api-security.js:44-60`).
- **Chat/TTS sem token:** qualquer processo local que alcance a porta pode gastar
  créditos de LLM/TTS. É o desenho atual, não um defeito novo.

### 6.2 Escrita destrutiva

- `POST /api/memory/forget` é imediato e **não tem desfazer**. A confirmação é
  apenas a palavra no corpo; um bug de UI que a envie sozinha apaga de verdade.
- `kind:"all"` é o mais perigoso: apaga fatos, recentes, resumos, histórico,
  afinidade e `user.createdAt`. Não é “limpar a lista”.
- Apagar `history`/`summary` altera a continuidade da persona nos próximos
  turnos (o prompt lê histórico do dia e os três resumos mais recentes,
  `server.mjs:522-532`).
- Excluir um fato não impede o modelo de aprendê-lo de novo no próximo chat.
- Duas janelas (web + desktop) podem escrever concorrentemente. Dentro de um
  processo o store é compartilhado, mas dois processos de API (dois `npm run api`)
  fazem last-write-wins no arquivo.
- O POST de personalidade substitui o objeto inteiro. Uma UI que envie só o slider
  alterado zera os demais traços para 50 (comportamento do `normalizePersonality`,
  não erro).

### 6.3 Dados sensíveis

- O snapshot de memória contém fatos pessoais, apelido, resumos de dias
  anteriores e datas. Não deve ir para log, telemetria, crash report ou arquivo
  em claro do desktop sem decisão explícita.
- O histórico de mensagens do servidor não é exposto (só contagem), mas os
  **resumos** carregam texto derivado da conversa.
- O chat publica `userMessage` e `reply` no webhook do n8n por padrão; há opção de
  omitir texto no servidor. Um botão de “teste” na aba CONVERSA dispararia esse
  fluxo.
- O TTS não expõe a voz configurada, mas o áudio sintetizado é dado derivado da
  fala; cache local guarda o áudio (`data/tts-cache/`).

### 6.4 Custo e limites

- **Chat:** cada chamada consome tokens de LLM e grava memória/afinidade; pode
  disparar uma chamada extra de resumo em background. Usar `/api/chat` como
  health check é caro e sujo.
- **TTS:** consome cota do provedor; é serializado (1 + fila 4); 429/503 sob
  carga; cache hit evita nova síntese, mas trocar voz/instrução invalida a chave
  (`CACHE_VERSION`).
- **STT:** roda whisper local (CPU/RAM), uma transcrição por vez, fila 2; 0,5 s de
  áudio levou ~15 s com o modelo frio.
- **Prompt:** os tetos de memória permitem um system prompt grande no pior caso:
  60 fatos × 300 + 30 recentes × 300 + 8 mensagens (até 2000 cada) + 3 resumos ×
  1200 + 600 de prompt ≈ 47 mil caracteres (~12 mil tokens) só de memória, além
  de persona/capacidades. O uso típico é muito menor, mas o teto existe
  (`src/memory-store.js:17-28`).
- **Limites de corpo:** memória/forget, chat e TTS 8 KiB; personalidade 4 KiB.
  `message`/`text` ≤ 2000; fato ≤ 300; resumo ≤ 1200; `reply` ≤ 220.

### 6.5 Contratos que a UI pode interpretar errado

- `removed: 0` em `kind:"all"` não significa “nada apagado”.
- POST de personalidade não devolve catálogo de traços nem `customPromptMax`.
- `affinity`/`stage` podem faltar na resposta do chat se a gravação falhar.
- `speechTags` é separado do `reply`; o desktop hoje ignora as tags.
- `GET /api/memory` exige token até para leitura; não há variante pública.
- O `source` do chat é `nvidia`, `openrouter` ou `offline` — não é o nome do
  modelo. Não há como obter o modelo carregado.

---

## 7. Decisões que são do Lucas

1. **Token no desktop:** variável de ambiente, arquivo local protegido, leitura do
   `.env` ou outro caminho? Sem isso, MEMÓRIA, JEITO e a voz da chamada ficam em
   401.
2. **Memória editável:** manter “inspecionar + excluir” (implementável hoje) ou
   autorizar uma rota nova de backend para criar/editar fatos? A rota não existe.
3. **CONVERSA:** mostrar apenas dado local + `source`/`timing`, ou autorizar um
   endpoint de saúde/config que hoje não existe?
4. **VOZ:** manter a voz/modelo como configuração exclusiva do servidor (`.env` +
   restart) ou expor leitura/escrita? Hoje não há endpoint.
5. **AVATAR:** o desktop deve ter configuração local própria ou espelhar o
   `localStorage` do web? Não há estado compartilhado no servidor.
6. **Vínculo:** a aba deve mostrar a afinidade do servidor (0–100, em
   `memory.json`) ou os medidores locais do desktop (carinho/humor/companhia)?
   São números diferentes.

---

## 8. Índice de evidências

### Backend (`usami/`)

| Assunto | Arquivo:linha |
| --- | --- |
| Roteador `/api` | `server.mjs:2400-2415` |
| Escuta em loopback | `server.mjs:2419` |
| Snapshot de memória | `server.mjs:1077-1091` |
| Confirmação/kinds de exclusão | `server.mjs:1093-1095` |
| Handler de memória (GET/POST) | `server.mjs:1147-1238` |
| Handler de personalidade | `server.mjs:1100-1145` |
| Parsers de corpo (chat/TTS) | `server.mjs:316-343` |
| Resposta do chat | `server.mjs:827-837` |
| Escrita de memória/afinidade no chat | `server.mjs:756-788` |
| Resumos em background | `server.mjs:793-801` |
| Webhook do n8n | `server.mjs:808-817` |
| Parsing/resposta do TTS | `server.mjs:1457-1697` |
| Headers do PCM em streaming | `server.mjs:1402-1407` |
| Token obrigatório no STT | `server.mjs:1809` |
| Resposta do STT | `server.mjs:1870` |
| `denyWithoutToken` | `server.mjs:278-283` |
| Host/Origin e token | `src/api-security.js:10,44-77` |
| Injeção do token no proxy do web | `vite.config.mjs:8,25` |
| Limites da memória | `src/memory-store.js:17-28` |
| Exclusões da memória | `src/memory-store.js:361-431,448-451` |
| Traços e limite do prompt | `src/personality-store.js:16-39,60-90` |
| Limite do reply | `src/provider-parse.js:20` |
| Estado do proativo | `src/proactive-engine.js:206-213` |
| Painel de memória do web | `src/Usami.jsx:691-733` |
| Painel de personalidade do web | `src/PersonalityPanel.jsx:14-50` |

### Desktop (`usami-desktop/`)

| Assunto | Arquivo:linha |
| --- | --- |
| Enum das abas | `src/main.rs:28-35` |
| Barra de abas | `src/main.rs:2104` |
| Conteúdo CONVERSA | `src/main.rs:2166-2206` |
| Conteúdo VOZ | `src/main.rs:2207-2248` |
| Conteúdo AVATAR | `src/main.rs:2249-2334` |
| Conteúdo MEMÓRIA | `src/main.rs:2335-2372` |
| Conteúdo JEITO | `src/main.rs:2373-2424` |
| Conteúdo SOBRE | `src/main.rs:2425+` |
| Cliente de chat sem token | `src/api.rs:114-163` |
| Cliente de TTS sem token | `src/api.rs:167-197` |
| Cliente de STT sem token | `src/api.rs:199-228` |
| `ChatResponse` sem `timing` | `src/api.rs:70-78` |
| Uso cru do `reply` no balão/TTS | `src/main.rs:732,798` |
| Histórico em memória | `src/main.rs:348` |
| Medidores locais | `src/main.rs:762-767` |

**Nota:** os números de linha do desktop refletem o working tree de 2026-11-08
com o WIP de presença; podem deslocar quando esse trabalho for commitado.

---

## 9. O que não foi feito

- Nenhuma alteração em `usami/` (código, dados, contratos ou `.env`).
- Nenhum commit/push em nenhum dos dois repositórios.
- Nenhum acesso a `vault/` ou `assets/`.
- Nenhuma chamada de chat contra a memória real (o chat real rodou só na
  instância isolada).
- Nenhum endpoint novo foi proposto como se existisse; as ausências estão
  marcadas como ausências.
