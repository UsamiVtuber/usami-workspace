# n8n local

This stack runs n8n and PostgreSQL on this computer only. Its editor is at
http://localhost:5678 and is not exposed to the local network.

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

## Before external access

The configuration intentionally binds n8n to `127.0.0.1`. Before making
webhooks externally reachable, replace the internal database password and set
an HTTPS public URL in `WEBHOOK_URL`; a secure tunnel is preferable to opening
a router port.
