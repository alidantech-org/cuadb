-- Permission-restricted catalog fixture. Run as an operator in a separate
-- cuadb_fixture_* database; do not grant schema usage or table access to the
-- reader used by CUADB_TEST_CATALOG_DENIED_URL.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing denied-catalog fixture outside cuadb_fixture_* database';
  END IF;
END
$guard$;

CREATE SCHEMA cuadb_fixture;
CREATE TABLE cuadb_fixture.hidden_stats (id integer PRIMARY KEY, value text);
INSERT INTO cuadb_fixture.hidden_stats VALUES (1, 'not visible to the reader');
