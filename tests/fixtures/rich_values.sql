-- Additive, idempotent fixture for rich-value rendering tests.
-- Run as the fixture operator in a database named cuadb_fixture_* ONLY.
-- Never drops or rewrites existing objects; rows are inserted only when empty.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing fixture setup outside cuadb_fixture_* database';
  END IF;
END
$guard$;
CREATE TABLE IF NOT EXISTS cuadb_fixture.rich_values (
  id integer PRIMARY KEY,
  doc jsonb,
  arr integer[],
  state cuadb_fixture.review_state,
  raw bytea,
  stamp timestamptz,
  exact numeric(30,10),
  markup text,
  uid uuid,
  addr inet
);
INSERT INTO cuadb_fixture.rich_values
SELECT 1,
       '{"a":{"b":[1,2,{"c":null}]},"s":"<script>alert(1)</script>"}'::jsonb,
       ARRAY[1,2,NULL,4],
       'approved',
       '\x00ff10'::bytea,
       '2026-01-02 03:04:05.123456+03'::timestamptz,
       12345678901234567890.1234567890,
       E'<script>alert(1)</script>\u0007‮ line\nbreak',
       '123e4567-e89b-12d3-a456-426614174000',
       '192.168.0.1/24'
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.rich_values WHERE id = 1);
INSERT INTO cuadb_fixture.rich_values
SELECT 2,
       (SELECT jsonb_agg(g) FROM generate_series(1,20000) g),
       (SELECT array_agg(g) FROM generate_series(1,20000) g),
       NULL,
       decode(repeat('ab',20000),'hex'),
       NULL, NULL, repeat('x',100000), NULL, NULL
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.rich_values WHERE id = 2);
ANALYZE cuadb_fixture.rich_values;
-- GRANT SELECT ON cuadb_fixture.rich_values TO <reader role>; (run by the owner)
