# n8n local

This stack runs n8n and PostgreSQL on this computer only. Its editor is at
http://localhost:5678 and is not exposed to the local network.

A stack que roda no notebook Nyarch (Arch) tem pasta e documentação próprias:
[`nyarch/`](nyarch/README.md).

## Start

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

A Usami e o n8n se comunicam em duas direções, ambas por endereço de laço
(`127.0.0.1`/`localhost`) ou do link direto entre as máquinas (`10.42.0.1`):

### Usami → n8n (respostas e eventos)

Cada resposta do chat (`POST /api/chat` na Usami) dispara um webhook
fire-and-forget para a URL em `N8N_WEBHOOK_URL` (hoje
`http://10.42.0.1:5678/webhook/usami-events`, lida do `.env` da raiz pelo
`server.mjs`; sem a variável o default é `localhost:5678`) com:

- `event` (`usami.response`), `timestamp`, `source` (`nvidia` ou `openrouter`)
- `userMessage`, `reply`, `gesture`, `actions` (animações com delayMs/trigger)
- `emotion`: `{ emotion, confidence, intensity, facs, source }` — o rótulo está
  em `emotion.emotion`; o campo vem `null` quando a classificação falha
- `affinity`: `{ affinity, stage }` — o score está em `affinity.affinity`; o
  objeto todo some quando não há store de memória

O workflow **Usami - Receber Respostas** (em `workflows/`) recebe esse webhook
e grava um Markdown no vault Obsidian em
`Inbox/Responses/`.

Para importar: no editor do n8n, vá em Workflows → Importar e selecione
`workflows/Usami - Receber Respostas.json`. O webhook ficará disponível em
`POST http://localhost:5678/webhook/usami-events`.

### n8n → Usami (controle de avatar)

A Usami expõe endpoints para o n8n enviar comandos de volta:

- `POST http://127.0.0.1:8787/api/n8n/command` — enfileira um comando
  - `{ "type": "say", "message": "texto" }` — faz a Usami dizer algo
  - `{ "type": "gesture", "name": "wave" }` — gesto do avatar
  - `{ "type": "set_expression", "name": "happy", "weight": 1 }` — expressão VRM
  - `{ "type": "set_morph", "name": "a", "value": 0.5 }` — morph target
  - `{ "type": "set_mesh_visible", "name": "cabelo", "visible": false }` — mostra/esconde mesh
- `GET http://127.0.0.1:8787/api/n8n/commands` — o cliente da Usami consome
  essa fila a cada 2s e aplica os comandos no avatar localmente.

Exemplo de nó HTTP Request no n8n para acionar um gesto:
- Método: POST
- URL: `http://127.0.0.1:8787/api/n8n/command`
- Body JSON: `{ "type": "gesture", "name": "wave" }`

## Antes de expor externamente

A configuração intencionalmente vincula o n8n a `127.0.0.1`. Antes de tornar
webhooks acessíveis externamente, troque a senha do banco interno e defina uma
URL HTTPS pública em `WEBHOOK_URL`; um túnel seguro é preferível a abrir uma
porta do roteador.
