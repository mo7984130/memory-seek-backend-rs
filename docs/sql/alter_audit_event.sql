-- ============================================
-- 通用事务审计事件表（只追加，不删除）
-- ============================================

CREATE TABLE IF NOT EXISTS audit_event
(
    event_id     BIGINT PRIMARY KEY,
    event_type   VARCHAR(120) NOT NULL,
    actor_id     BIGINT NULL,
    target_type  VARCHAR(64) NULL,
    target_id    BIGINT NULL,
    detail       JSONB NULL,
    occurred_at  TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_event_type_occurred
    ON audit_event (event_type, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_event_actor_occurred
    ON audit_event (actor_id, occurred_at DESC);

COMMENT ON TABLE audit_event IS '事务强一致审计事件表(只追加,不删除)';
