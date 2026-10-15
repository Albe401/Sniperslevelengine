CREATE SCHEMA IF NOT EXISTS snipers_snapshots;
CREATE TABLE IF NOT EXISTS snipers_snapshots.batches (
 id uuid PRIMARY KEY, writer_id text NOT NULL, slot_ms bigint NOT NULL,
 captured_ms bigint NOT NULL, stored_at timestamptz NOT NULL DEFAULT now(),
 schema_version text NOT NULL, source text NOT NULL, asset_count integer NOT NULL CHECK(asset_count=10),
 sha256 text NOT NULL, coverage jsonb NOT NULL, extras jsonb NOT NULL,
 UNIQUE(writer_id,slot_ms), CHECK(captured_ms >= slot_ms)
);
CREATE TABLE IF NOT EXISTS snipers_snapshots.assets (
 batch_id uuid NOT NULL REFERENCES snipers_snapshots.batches(id),
 symbol text NOT NULL CHECK(symbol IN ('AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM')),
 published_ms bigint, payload jsonb NOT NULL, sections jsonb NOT NULL,
 PRIMARY KEY(batch_id,symbol)
);
CREATE INDEX IF NOT EXISTS snapshot_capture_desc ON snipers_snapshots.batches(captured_ms DESC);
CREATE INDEX IF NOT EXISTS snapshot_asset_batch ON snipers_snapshots.assets(symbol,batch_id);
ALTER TABLE snipers_snapshots.assets ADD COLUMN IF NOT EXISTS payload_compressed bytea;
ALTER TABLE snipers_snapshots.assets ADD COLUMN IF NOT EXISTS payload_sha256 text;
ALTER TABLE snipers_snapshots.assets ADD COLUMN IF NOT EXISTS payload_codec text;
CREATE TABLE IF NOT EXISTS snipers_snapshots.images (
 batch_id uuid NOT NULL REFERENCES snipers_snapshots.batches(id),
 symbol text NOT NULL, mime_type text NOT NULL CHECK(mime_type='image/jpeg'),
 image bytea NOT NULL, image_sha256 text NOT NULL, source_sha256 text NOT NULL,
 generated_ms bigint NOT NULL, renderer_version text NOT NULL,
 PRIMARY KEY(batch_id,symbol), FOREIGN KEY(batch_id,symbol) REFERENCES snipers_snapshots.assets(batch_id,symbol)
);
COMMENT ON SCHEMA snipers_snapshots IS 'Point-in-time Snipers observations. Missing data remains missing. Not a complete tick archive.';
