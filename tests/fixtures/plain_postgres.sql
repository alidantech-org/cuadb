-- Disposable PostgreSQL fixture without PostGIS.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing plain PostgreSQL fixture outside cuadb_fixture_* database';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'postgis') THEN
    RAISE EXCEPTION 'This fixture must not have PostGIS installed';
  END IF;
END
$guard$;

CREATE SCHEMA IF NOT EXISTS cuadb_fixture;
CREATE TABLE IF NOT EXISTS cuadb_fixture.plain_probe (
  id integer PRIMARY KEY,
  label text NOT NULL
);
INSERT INTO cuadb_fixture.plain_probe(id, label)
VALUES (1, 'plain PostgreSQL')
ON CONFLICT (id) DO NOTHING;
