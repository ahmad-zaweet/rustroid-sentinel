CREATE TABLE IF NOT EXISTS alerts (
    id UUID PRIMARY KEY,
    approach_id UUID NOT NULL REFERENCES approaches(id) ON DELETE CASCADE,
    alert_type TEXT NOT NULL,
    alerted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB,
    UNIQUE (approach_id, alert_type)
);

CREATE INDEX IF NOT EXISTS idx_alerts_approach_id ON alerts(approach_id);
