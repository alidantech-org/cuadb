-- Optional addition for a disposable PostGIS-enabled fixture only.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing PostGIS fixture outside cuadb_fixture_* database';
  END IF;
END
$guard$;
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE TABLE cuadb_fixture.places (
    id integer PRIMARY KEY,
    location geometry(Point,4326)
);
CREATE INDEX places_gist ON cuadb_fixture.places USING gist(location);
INSERT INTO cuadb_fixture.places (id,location)
VALUES (1, ST_SetSRID(ST_MakePoint(36.82,-1.29),4326));
ANALYZE cuadb_fixture.places;
-- Reapply SELECT grants to cuadb_reader for the new table.
