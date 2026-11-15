# n8n local

This stack runs n8n and PostgreSQL on this computer. Its editor is at
http://localhost:5678, but the compose publishes the port as `0.0.0.0:5678:5678`:
anything on the local network can reach the editor. The bind restriction
(`10.42.0.1`/`127.0.0.1`) exists only in the Nyarch mirror — see
[`nyarch/README.md`](nyarch/README.md).

A stack que roda no notebook Nyarch (Arch) tem pasta e documentação próprias:
[`nyarch/`](nyarch/README.md).

## Start

A senha do banco vem de `.env` nesta pasta (copie `.env.example`, preencha). O
compose não carrega o `.env` da raiz do workspace — ele procura `.env` ao lado do
arquivo, porque os comandos rodam a partir daqui. Sem a variável, o compose falha
na hora com `${POSTGRES_PASSWORD:?...}` em vez de subir com senha vazia.

From this directory, after Docker Desktop reports that its engine is running:

```powershell
docker compose up -d
docker compose ps
```

On the first visit to http://localhost:5678, create the n8n owner account.

## Stop and inspect

```powershell
docker compose stop
docker compose logs -f n8n
```

The named volumes `n8n_data` and `n8n_postgres_data` retain credentials and
workflows across restarts. Do not run `docker compose down -v` unless the local
n8n installation is intentionally being erased.

## Integração Usami ↔ n8n

A Usami e o n8n se comunicam em duas direções, por endereço de laço
(`127.0.0.1`/`localhost`) ou pelo link direto entre as máquinas: `10.42.0.1` é o
notebook Nyarch (n8n) e `10.42.0.91` é o Windows (API da Usami):

### Usami → n8n (respostas e eventos)

Cada resposta do chat (`POST /api/chat` na Usami) dispara um webhook
fire-and-forget para a URL em `N8N_WEBHOOK_URL` (hoje
`http://10.42.0.1:5678/webhook/usami-events`, lida do `.env` da raiz pelo
`server.mjs`; sem a variável o default é `localhost:5678`) com:

- `event` (`usami.response`), `timestamp`, `source` (`nvidia`, `openrouter`,
  `gemini` ou `offline`)
- `userMessage`, `reply`, `gesture`, `actions` (animações com delayMs/trigger)
- `emotion`: `{ emotion, confidence, intensity, facs, source }` — o rótulo está
  em `emotion.emotion`; o campo vem `null` quando a classificação falha
- `affinity`: `{ affinity, stage }` — o score está em `affinity.affinity`; o
  objeto todo some quando não há store de memória

Com `N8N_WEBHOOK_SECRET` no `.env` o corpo sai assinado (`X-Usami-Timestamp` +
`X-Usami-Signature`, HMAC-SHA256 de `<timestamp>.<corpo>`); `N8N_WEBHOOK_OMIT_TEXT=true`
remove `userMessage`/`reply` do evento.

O workflow **Usami - Receber Respostas** (em `workflows/`) recebe esse webhook
e grava um Markdown no vault Obsidian em
`Inbox/Responses/`.

Para importar: no editor do n8n, vá em Workflows → Importar e selecione
`workflows/Usami - Receber Respostas.json`. O webhook ficará disponível em
`POST http://localhost:5678/webhook/usami-events`. O workflow já está publicado
e ativo na instância do Nyarch (`bJJ17mDS2I5r4De9`); importar o JSON cria uma
segunda cópia com outros ids (ver `workflows/README.md`).

### n8n → Usami (controle de avatar)

A Usami expõe endpoints para o n8n enviar comandos de volta. A API escuta em
`0.0.0.0:8787` por padrão (`USAMI_API_HOST`), então o n8n do Nyarch a alcança em
`http://10.42.0.91:8787`; da própria máquina, `127.0.0.1:8787`. As rotas de fila
passam pela allowlist de Host/Origin e **sempre exigem `x-usami-token`** — sem
`USAMI_API_TOKEN` no `.env` elas respondem 503 (fail-closed); com o token
definido, use a credencial HTTP Header Auth do n8n.

- `POST /api/n8n/command` — enfileira um comando
  - `{ "type": "say", "message": "texto" }` — faz a Usami dizer algo
  - `{ "type": "proactive_say", "message": "texto" }` — fala proativa (balão + áudio direto no cliente)
  - `{ "type": "gesture", "name": "wave" }` — gesto do avatar (nome precisa estar em `GESTURE_NAMES`)
  - `{ "type": "set_expression", "name": "happy", "weight": 1 }` — expressão VRM
  - `{ "type": "set_morph", "name": "a", "value": 0.5 }` — morph target
  - `{ "type": "set_mesh_visible", "name": "cabelo", "visible": false }` — mostra/esconde mesh
  - `{ "type": "play_sequence", "shape": "...", ... }` — sequência a partir de shape paramétrica
- `GET /api/n8n/commands` — o cliente da Usami consome essa fila a cada 2 s
  (`x-usami-client` identifica a aba) e aplica os comandos no avatar localmente.
  O GET **marca como entregue e não reentrega**; o cliente confirma com
  `POST /api/n8n/commands/ack` (`{ "ids": [...] }`). A fila é em memória, por
  cliente (máx. 50), e reiniciar a API descarta pendentes.
- `DELETE /api/n8n/commands` — o cliente descarta a própria fila.

Exemplo de nó HTTP Request no n8n para acionar um gesto:
- Método: POST
- URL: `http://10.42.0.91:8787/api/n8n/command`
- Credencial: HTTP Header Auth com `x-usami-token`
- Body JSON: `{ "type": "gesture", "name": "wave" }`

## Antes de expor externamente

O compose local não restringe o bind: o editor fica em `0.0.0.0:5678`, visível
para a rede local. (O espelho do Nyarch é que publica apenas `10.42.0.1:5678` e
`127.0.0.1:5678`.) Antes de tornar webhooks acessíveis externamente, troque a
senha do banco interno e defina uma URL HTTPS pública em `WEBHOOK_URL`; um túnel
seguro é preferível a abrir uma porta do roteador. Trocar a senha é dois passos:
o `postgres:16-alpine` só usa `POSTGRES_PASSWORD` ao inicializar um volume vazio,
então mude o `.env` **e** rode `ALTER USER n8n WITH PASSWORD ...` no banco
existente — um sem o outro derruba a conexão do n8n.
