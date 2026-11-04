-- Add to the opt-in, disposable cuadb_fixture_* catalog database.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing fixture setup outside cuadb_fixture_* database';
  END IF;
END
$guard$;
-- Never run in any existing application or personal database.
CREATE TABLE cuadb_fixture.empty_paging (id bigint PRIMARY KEY, note text);
CREATE TABLE cuadb_fixture.single_paging (id bigint PRIMARY KEY, note text);
INSERT INTO cuadb_fixture.single_paging VALUES (1,'only row');
CREATE TABLE cuadb_fixture.paging (
  id bigint PRIMARY KEY,
  amount numeric(12,3),
  nullable_text text,
  created_at timestamptz NOT NULL DEFAULT now(),
  binary_data bytea
);
INSERT INTO cuadb_fixture.paging (id,amount,nullable_text,binary_data)
SELECT n,n::numeric/10,
       CASE WHEN n % 5 = 0 THEN NULL WHEN n=1 THEN repeat('Ω',1000)
            ELSE ('group-' || (n%3)::text) END,
       CASE WHEN n=2 THEN decode('00ff','hex') ELSE NULL END
FROM generate_series(1,120) AS n;
ANALYZE cuadb_fixture.paging;
