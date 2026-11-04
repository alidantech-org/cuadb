-- Additive, idempotent large-table fixture for latency/timeout checks (PostgreSQL).
-- Run as the fixture operator inside a database named cuadb_fixture_* ONLY. ~1,000,000 rows.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing fixture setup outside cuadb_fixture_* database';
  END IF;
END
$guard$;
CREATE TABLE IF NOT EXISTS cuadb_fixture.perf_big (
  id bigint PRIMARY KEY,
  grp integer NOT NULL,
  payload text NOT NULL,
  created_at timestamptz NOT NULL
);
INSERT INTO cuadb_fixture.perf_big
SELECT n, n % 1000, md5(n::text), timestamptz '2026-01-01' + (n || ' seconds')::interval
FROM generate_series(1, 1000000) AS n
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.perf_big);
ANALYZE cuadb_fixture.perf_big;
-- GRANT SELECT ON cuadb_fixture.perf_big TO <reader role>; (run by the owner)
