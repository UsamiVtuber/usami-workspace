# Observabilidade Local — Langfuse para Usami

Esta pasta contém o stack Docker para rodar o Langfuse localmente, garantindo que 100% dos traces, métricas e auditorias de IA da Usami fiquem na sua própria máquina, sem enviar dados para a nuvem.

## Como iniciar

```bash
docker compose up -d
```

A interface web estará disponível em:
👉 **http://localhost:3000**

No primeiro acesso:
1. Crie sua conta local de administrador (apenas para acesso local na máquina).
2. Crie um projeto chamado **Usami**.
3. Em *Settings* -> *API Keys*, gere uma chave (Public Key e Secret Key).
4. Adicione no arquivo `.env` da Usami (`usami/.env`):
   ```env
   LANGFUSE_PUBLIC_KEY=pk-lf-...
   LANGFUSE_SECRET_KEY=sk-lf-...
   LANGFUSE_HOST=http://localhost:3000
   ```

## Parar os serviços

```bash
docker compose down
```
Os dados dos traces e dashboards ficam salvos de forma persistente no volume Docker `langfuse_postgres_data`.

