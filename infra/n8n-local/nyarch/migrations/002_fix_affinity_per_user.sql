-- Migration 002: Fix affinity to be per-user (earned, not set)

-- Drop old table and create new per-user affinity table
DROP TABLE IF EXISTS usami_affinity;

CREATE TABLE usami_affinity (
  guild_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN ('Conhecida', 'Amiga', 'Amiga Íntima', 'Confidente', 'Alma Gêmea')),
  points INT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (guild_id, user_id)
);

CREATE INDEX idx_usami_affinity_guild ON usami_affinity (guild_id);

-- Function to get user affinity
CREATE OR REPLACE FUNCTION get_user_affinity(p_guild_id TEXT, p_user_id TEXT)
RETURNS JSONB AS $$
DECLARE
  v_result JSONB;
BEGIN
  SELECT jsonb_build_object(
    'stage', COALESCE(stage, 'Conhecida'),
    'points', COALESCE(points, 0),
    'updated_at', updated_at
  ) INTO v_result
  FROM usami_affinity
  WHERE guild_id = p_guild_id AND user_id = p_user_id;
  RETURN COALESCE(v_result, jsonb_build_object('stage', 'Conhecida', 'points', 0));
END;
$$ LANGUAGE plpgsql;