# Observabilidade Local — Langfuse para Usami

Esta pasta contém o stack Docker para rodar o Langfuse localmente: traces do turno de chat, spans de recuperação de memória e de ferramentas e as gerações de LLM da Usami ficam na sua própria máquina. Os dois serviços escutam apenas no loopback (`127.0.0.1`) e a telemetria do próprio Langfuse está desligada (`TELEMETRY_ENABLED: "false"` no compose).

## Como iniciar

A partir da raiz do workspace (o compose precisa ser lido da pasta dele):

```bash
cd infra/observability
docker compose up -d
```

Sobe o `langfuse/langfuse:2` com um Postgres 16 ao lado. A interface web estará disponível em:
👉 **http://localhost:3000**

O Postgres fica exposto só em `127.0.0.1:5433`, para inspeção local. As senhas têm valores de desenvolvimento embutidos no compose; para sobrescrever, crie um `.env` nesta pasta com `LANGFUSE_POSTGRES_PASSWORD`, `LANGFUSE_NEXTAUTH_SECRET` e `LANGFUSE_SALT`.

No primeiro acesso:
1. Crie sua conta local de administrador (apenas para acesso local na máquina).
2. Crie um projeto chamado **Usami**.
3. Em *Settings* -> *API Keys*, gere uma chave (Public Key e Secret Key).
4. Exporte as variáveis no ambiente do processo que roda a API (a partir de `usami/`) e suba com `npm run api`:
   ```powershell
   $env:LANGFUSE_PUBLIC_KEY="pk-lf-..."
   $env:LANGFUSE_SECRET_KEY="sk-lf-..."
   $env:LANGFUSE_HOST="http://localhost:3000"
   npm run api
   ```
   No bash/Git Bash, use `export LANGFUSE_HOST=...`. `usami/src/telemetry.js` lê essas três variáveis direto de `process.env`; o `.env` do projeto (na raiz do workspace) não é carregado no ambiente do processo, então exportá-las antes de subir a API é o que liga a telemetria. Sem elas, tudo vira no-op e o chat continua funcionando.

## Parar os serviços

```bash
docker compose down
```
Os dados dos traces e dashboards ficam salvos de forma persistente no volume declarado no compose, `langfuse_postgres_data` (o Docker prefixa com o nome do projeto — por padrão, `observability_langfuse_postgres_data`). Não use `-v` ao derrubar se quiser manter contas, chaves e histórico.
