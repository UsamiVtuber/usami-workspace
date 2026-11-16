-- Schema de inicialização idempotente para Observabilidade e Bot Discord da Usami.
-- Executado automaticamente pelo PostgreSQL na criação inicial do volume (/docker-entrypoint-initdb.d/).

-- ============================================================================
-- 1. OBSERVABILIDADE (Chamadas API Usami, Erros e Execuções n8n)
-- ============================================================================

CREATE TABLE IF NOT EXISTS obs_api_logs (
  id BIGSERIAL PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  event_type TEXT NOT NULL DEFAULT 'api_call',
  method TEXT,
  route TEXT,
  status_code INT,
  duration_ms INT,
  provider TEXT,
  model TEXT,
  client_id TEXT,
  is_error BOOLEAN NOT NULL DEFAULT FALSE,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_obs_api_logs_created_at ON obs_api_logs (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_obs_api_logs_status_code ON obs_api_logs (status_code);
CREATE INDEX IF NOT EXISTS idx_obs_api_logs_route ON obs_api_logs (route);

CREATE TABLE IF NOT EXISTS obs_error_logs (
  id BIGSERIAL PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  source TEXT NOT NULL,
  route_or_workflow TEXT,
  status_code INT,
  error_code TEXT,
  error_message TEXT NOT NULL,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_obs_error_logs_created_at ON obs_error_logs (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_obs_error_logs_source ON obs_error_logs (source);

CREATE TABLE IF NOT EXISTS obs_n8n_executions (
  execution_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  workflow_id TEXT,
  workflow_name TEXT,
  status TEXT NOT NULL,
  mode TEXT,
  duration_ms INT,
  error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_obs_n8n_executions_created_at ON obs_n8n_executions (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_obs_n8n_executions_workflow_id ON obs_n8n_executions (workflow_id);

-- ============================================================================
-- 2. BOT DISCORD DA USAMI (Funcionalidades inspiradas em Rukafuu/LiraVtuber)
-- ============================================================================

CREATE TABLE IF NOT EXISTS discord_afk (
  user_id TEXT PRIMARY KEY,
  guild_id TEXT NOT NULL,
  username TEXT NOT NULL,
  reason TEXT NOT NULL DEFAULT 'Ausente no momento',
  since_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS discord_birthdays (
  user_id TEXT PRIMARY KEY,
  guild_id TEXT NOT NULL,
  username TEXT NOT NULL,
  birth_day INT NOT NULL CHECK (birth_day BETWEEN 1 AND 31),
  birth_month INT NOT NULL CHECK (birth_month BETWEEN 1 AND 12),
  last_announced_year INT
);

CREATE INDEX IF NOT EXISTS idx_discord_birthdays_month_day ON discord_birthdays (birth_month, birth_day);

CREATE TABLE IF NOT EXISTS discord_notes (
  id BIGSERIAL PRIMARY KEY,
  user_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_discord_notes_user_id ON discord_notes (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS discord_suggestions (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  username TEXT NOT NULL,
  content TEXT NOT NULL,
  message_id TEXT,
  status TEXT NOT NULL DEFAULT 'open',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS discord_giveaways (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  channel_id TEXT NOT NULL,
  message_id TEXT,
  prize TEXT NOT NULL,
  winners_count INT NOT NULL DEFAULT 1,
  ends_at TIMESTAMPTZ NOT NULL,
  ended BOOLEAN NOT NULL DEFAULT FALSE,
  host_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_discord_giveaways_active ON discord_giveaways (ended, ends_at);

CREATE TABLE IF NOT EXISTS discord_tickets (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  channel_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  username TEXT NOT NULL,
  topic TEXT NOT NULL DEFAULT 'Suporte',
  status TEXT NOT NULL DEFAULT 'open',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  closed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_discord_tickets_channel_status ON discord_tickets (channel_id, status);

CREATE TABLE IF NOT EXISTS discord_reaction_roles (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  channel_id TEXT NOT NULL,
  message_id TEXT NOT NULL,
  emoji TEXT NOT NULL,
  role_id TEXT NOT NULL,
  UNIQUE (message_id, emoji)
);

CREATE TABLE IF NOT EXISTS discord_sticky_messages (
  channel_id TEXT PRIMARY KEY,
  guild_id TEXT NOT NULL,
  content TEXT NOT NULL,
  last_message_id TEXT,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS discord_custom_commands (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  command_name TEXT NOT NULL,
  response_text TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (guild_id, command_name)
);

CREATE TABLE IF NOT EXISTS discord_mod_logs (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  moderator_id TEXT NOT NULL,
  action TEXT NOT NULL,
  reason TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_discord_mod_logs_guild_created ON discord_mod_logs (guild_id, created_at DESC);

CREATE TABLE IF NOT EXISTS discord_server_stats (
  guild_id TEXT PRIMARY KEY,
  messages_count BIGINT NOT NULL DEFAULT 0,
  commands_count BIGINT NOT NULL DEFAULT 0,
  members_joined BIGINT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
