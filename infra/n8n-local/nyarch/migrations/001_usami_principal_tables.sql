-- Migration 001: Tabelas para o comando /usami principal
-- Adiciona suporte a humor, afinidade, memória, modo sono e rich presence

-- ============================================================================
-- HUMOR DA USAMI
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_mood (
  guild_id TEXT PRIMARY KEY,
  mood TEXT NOT NULL CHECK (mood IN ('happy', 'tsundere', 'surprised', 'thinking', 'neutral', 'sad', 'angry')),
  set_by_user_id TEXT NOT NULL,
  set_by_username TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usami_mood_updated_at ON usami_mood (updated_at DESC);

-- ============================================================================
-- AFINIDADE DA USAMI
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_affinity (
  guild_id TEXT PRIMARY KEY,
  stage TEXT NOT NULL CHECK (stage IN ('Conhecida', 'Amiga', 'Amiga Íntima', 'Confidente', 'Alma Gêmea')),
  set_by_user_id TEXT NOT NULL,
  set_by_username TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usami_affinity_updated_at ON usami_affinity (updated_at DESC);

-- ============================================================================
-- MEMÓRIA DE LONGO PRAZO DA USAMI
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_memory (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  created_by_user_id TEXT NOT NULL,
  created_by_username TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (guild_id, key)
);

CREATE INDEX IF NOT EXISTS idx_usami_memory_guild_key ON usami_memory (guild_id, key);
CREATE INDEX IF NOT EXISTS idx_usami_memory_updated_at ON usami_memory (updated_at DESC);

-- ============================================================================
-- MODO SONO (COMPANHIA PARA DORMIR)
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_sleep_mode (
  guild_id TEXT PRIMARY KEY,
  channel_id TEXT NOT NULL,
  host_user_id TEXT NOT NULL,
  host_username TEXT NOT NULL,
  active BOOLEAN NOT NULL DEFAULT FALSE,
  ends_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usami_sleep_mode_active ON usami_sleep_mode (active, ends_at);

-- ============================================================================
-- RICH PRESENCE DA USAMI
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_rich_presence (
  guild_id TEXT PRIMARY KEY,
  state TEXT,
  details TEXT,
  large_image_key TEXT,
  large_image_text TEXT,
  small_image_key TEXT,
  small_image_text TEXT,
  start_timestamp BIGINT,
  end_timestamp BIGINT,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- LOG DE CHAT DO USAMI (para auditoria e contexto)
-- ============================================================================

CREATE TABLE IF NOT EXISTS usami_chat_log (
  id BIGSERIAL PRIMARY KEY,
  guild_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  username TEXT NOT NULL,
  message TEXT NOT NULL,
  thread_created BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usami_chat_log_guild_created ON usami_chat_log (guild_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_usami_chat_log_user_created ON usami_chat_log (user_id, created_at DESC);

-- ============================================================================
-- FUNÇÕES AUXILIARES
-- ============================================================================

-- Função para limpar modo sono expirado (pode ser chamada via cron)
CREATE OR REPLACE FUNCTION cleanup_expired_sleep_mode()
RETURNS INTEGER AS $$
DECLARE
  deleted_count INTEGER;
BEGIN
  DELETE FROM usami_sleep_mode
  WHERE active = TRUE AND ends_at IS NOT NULL AND ends_at < NOW();
  GET DIAGNOSTICS deleted_count = ROW_COUNT;
  RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Função para obter estatísticas completas da Usami
CREATE OR REPLACE FUNCTION get_usami_stats(p_guild_id TEXT)
RETURNS JSONB AS $$
DECLARE
  v_result JSONB;
BEGIN
  SELECT jsonb_build_object(
    'server', (SELECT row_to_json(s) FROM discord_server_stats s WHERE s.guild_id = p_guild_id),
    'mood', (SELECT row_to_json(m) FROM usami_mood m WHERE m.guild_id = p_guild_id ORDER BY m.updated_at DESC LIMIT 1),
    'affinity', (SELECT row_to_json(a) FROM usami_affinity a WHERE a.guild_id = p_guild_id ORDER BY a.updated_at DESC LIMIT 1),
    'sleepMode', (SELECT row_to_json(s) FROM usami_sleep_mode s WHERE s.guild_id = p_guild_id AND s.active = TRUE LIMIT 1),
    'presence', (SELECT jsonb_build_object(
        'state', COALESCE(rp.state, 'idle'),
        'details', COALESCE(rp.details, ''),
        'largeImage', COALESCE(rp.large_image_key, ''),
        'largeText', COALESCE(rp.large_image_text, ''),
        'smallImage', COALESCE(rp.small_image_key, ''),
        'smallText', COALESCE(rp.small_image_text, ''),
        'startTimestamp', rp.start_timestamp,
        'endTimestamp', rp.end_timestamp
      ) FROM usami_rich_presence rp WHERE rp.guild_id = p_guild_id ORDER BY rp.updated_at DESC LIMIT 1),
    'memoryCount', (SELECT COUNT(*) FROM usami_memory WHERE guild_id = p_guild_id),
    'chatLogCount', (SELECT COUNT(*) FROM usami_chat_log WHERE guild_id = p_guild_id)
  ) INTO v_result;
  
  RETURN COALESCE(v_result, '{}'::jsonb);
END;
$$ LANGUAGE plpgsql;