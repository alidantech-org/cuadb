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

-- Additive spatial edge cases (idempotent; run as the fixture operator):
CREATE TABLE IF NOT EXISTS cuadb_fixture.spatial_edges (
  id integer PRIMARY KEY,
  geom geometry,
  geog geography
);
INSERT INTO cuadb_fixture.spatial_edges
SELECT 1, ST_SetSRID(ST_MakePoint(4100000, -145000), 3857), NULL
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.spatial_edges WHERE id = 1);
INSERT INTO cuadb_fixture.spatial_edges
SELECT 2, ST_GeomFromText('POINT EMPTY', 4326), NULL
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.spatial_edges WHERE id = 2);
INSERT INTO cuadb_fixture.spatial_edges
SELECT 3, ST_SetSRID(ST_Buffer(ST_MakePoint(0,0), 10, 2500), 4326), NULL
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.spatial_edges WHERE id = 3);
INSERT INTO cuadb_fixture.spatial_edges
SELECT 4, NULL, ST_GeogFromText('SRID=4326;POINT(36.82 -1.29)')
WHERE NOT EXISTS (SELECT 1 FROM cuadb_fixture.spatial_edges WHERE id = 4);
-- GRANT SELECT ON cuadb_fixture.spatial_edges TO <reader role>; (run by the owner)
