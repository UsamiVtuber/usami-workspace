# n8n no Nyarch

Espelho versionável da stack que roda no notebook Arch (o "Nyarch", `10.42.0.1` no cabo
cruzado). O arquivo executado fica em `~/n8n/docker-compose.yml` no notebook; esta cópia
existe para o histórico e para reconstruir a máquina do zero. Ao editar, atualize os dois
lados e confirme com diff.

Esta pasta **não** é a stack local do Windows (`../docker-compose.yml`, n8n + PostgreSQL
publicado em `0.0.0.0:5678`) nem substitui o `README.md` da pasta pai.

## Operar do Windows

O acesso é pelo contexto de Docker sobre SSH (a chave está no `~/.ssh/config` como
`nyarch-docker`):

```powershell
docker context use nyarch
docker ps                                  # espera n8n-usami-inbox, n8n-searxng, n8n-usami-postgres, n8n-discord-bridge
ssh nyarch-docker "cd ~/n8n && docker compose up -d"
```

Editor: http://10.42.0.1:5678

## Dependências antes de recriar

1. **Stack do code sandbox** — a rede `n8n-sandbox_sandbox-service` é externa; se ela não
   existir, `docker compose up` falha. Suba antes:
   `ssh nyarch-docker "cd ~/n8n-sandbox && docker compose up -d"`.
   O runner usa `--privileged` porque `sysbox-runc` não funciona em Arch. O painel do
   sandbox só escuta em `127.0.0.1:8080`.
2. **Mount SMB do vault** — `/mnt/usami-automations` é montado por `usami-n8n-mount.service`
   com `fstab noauto` e entra no container como `/home/node/vault/Inbox/Automations`. Se
   estiver desmontado, o container vê a pasta vazia e os workflows que gravam ali (a versão
   ativa do `Daily Briefing`, via `$env.USAMI_VAULT_PATH`) escrevem no vazio: confira
   `ssh nyarch-docker "mountpoint /mnt/usami-automations"` antes de recriar. Já
   `/files/obsidian/usami` (bind de `/dados/obsidian/usami`) é o alvo do workflow de
   respostas (`Inbox/Responses`) e das automações que gravam em `Inbox/Automations` por
   esse caminho.
3. **`~/n8n/.env`** — veja `.env.example`. Os segredos nunca entram neste repositório.
4. **API da Usami no Windows** — de dentro do n8n use sempre `http://10.42.0.1:8787`
   (com credencial `Usami Token` nos endpoints protegidos). Esse é o
   `usami-api-bridge.service`, systemd de usuário no Nyarch que escuta em
   `10.42.0.1:8787` e repassa para `10.42.0.91:8787` pela interface física
   `enp2s0`. Chamar o IP do Windows direto (`.91`) de dentro do container falha
   com `ECONNREFUSED` — foi o que derrubou as execuções de 2026-11-17 às 04:00 —
   por isso os workflows agendados e o Chat Companion
   (`http://10.42.0.1:8787/api/chat`) usam o bridge.
   Se a API estiver parada, os nós HTTP falham conforme o `onError` de cada workflow.

## Travas conhecidas

- **Os três bind mounts do serviço `n8n` são obrigatórios.** Sem eles o n8n sobe com
  aparência normal em um banco vazio dentro da camada efêmera do container, rodando
  `InitialMigration`, e os workflows/credenciais reais ficam intactos no host porém
  invisíveis. Aconteceu de verdade em 2026-10-25. Prova de montagem correta: o inode de
  `/home/node/.n8n/database.sqlite` dentro do container é igual ao de
  `/dados/n8n/data/database.sqlite` no host.
- **Não valide o compose com grep no YAML renderizado.** `docker compose config` normaliza
  `networks: [a, b]` em mapeamento, então a contagem sai errada. Use
  `docker compose config --format json | jq` e confira redes, mounts e a interpolação dos
  segredos.
- **Nunca `docker compose down -v` nem apagar volume.** Os dados vivem em bind mounts sob
  `/dados/`; backups em `~/n8n-backups/`.

## n8n Assistant (sandbox + busca web)

As duas partes têm fontes de verdade diferentes, e a diferença é intencional:

- **Sandbox: pela env** (`N8N_INSTANCE_AI_SANDBOX_*`). Enquanto ela existir, a UI de
  "Add a code sandbox" responde **"Cannot update environment-managed fields"** — esperado,
  e o wizard aceita o sandbox como já configurado.
- **Busca web: pela UI**, salva como credencial `searXngApi` ("n8n Assistant web search")
  com o valor `http://searxng:8080`. **Não readicione `N8N_INSTANCE_AI_SEARXNG_URL` ao
  compose.** Em `instance-ai-settings.service.js`, `hasEnvironmentSearchConnection()` retorna
  true quando `searxngUrl` vem da env, e nesse caso `rejectEnvironmentManagedFields()` passa a
  recusar `searchConnection`, `searchDisabled` e `searchCredentialId` — o passo de web search
  do "Finish setup" fica insatisfitável. A env esteve lá em 2026-10-25 e foi removida por isso.

O SearXNG não publica porta (só o nome de serviço interno) e o `searxng-settings.yml` liga
`format=json`, sem o qual o nó de busca do n8n não funciona. Ele consulta buscadores
públicos, então gera tráfego de saída do notebook.

## Exposição de rede

Escuta esperada em `ss -tlnp`: `10.42.0.1:5678`, `127.0.0.1:5678`, `127.0.0.1:8080`. Nada
além da rede do cabo cruzado (`10.42.0.0/24`). Antes de publicar webhooks externamente,
troque `WEBHOOK_URL` para HTTPS por túnel seguro; não abra porta do roteador.

## PostgreSQL de aplicação e Discord Gateway Bridge

- **`n8n-usami-postgres`** (`postgres:16-alpine`): banco exclusivo da aplicação (tabelas de observabilidade `obs_*` e estado do bot Discord `discord_*` inicializadas por `init-db.sql`). O serviço `n8n` **não** usa `DB_TYPE=postgresdb` e permanece em `/home/node/.n8n/database.sqlite` para preservar credenciais e workflows existentes.
- Antes do primeiro `docker compose up -d`, garanta a pasta `/dados/n8n/postgres` no Nyarch (`mkdir -p /dados/n8n/postgres`) e defina `USAMI_POSTGRES_PASSWORD` em `~/n8n/.env`.
- **`n8n-discord-bridge`** (`./discord-bridge`): conecta ao Discord Gateway via WebSocket (outbound) e repassa mensagens, interações Slash Command, reações e entradas de membros para os webhooks internos do n8n (`http://n8n:5678/webhook/...`), sem expor portas.
