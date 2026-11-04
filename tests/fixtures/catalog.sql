-- DESTRUCTIVE FIXTURE. psql -v ON_ERROR_STOP=1 in a dedicated
-- database named cuadb_fixture_* ONLY. Never use a personal or production DB.
DO $guard$
BEGIN
  IF pg_catalog.current_database() !~ '^cuadb_fixture_' THEN
    RAISE EXCEPTION 'Refusing fixture setup outside cuadb_fixture_* database';
  END IF;
END
$guard$;
DROP SCHEMA IF EXISTS cuadb_fixture CASCADE;
DROP SCHEMA IF EXISTS "CuaDB Mixed" CASCADE;
CREATE SCHEMA cuadb_fixture;
CREATE SCHEMA "CuaDB Mixed";
CREATE TYPE cuadb_fixture.review_state AS ENUM ('new','approved');
CREATE DOMAIN cuadb_fixture.nonnegative AS integer CHECK (VALUE >= 0);

CREATE TABLE cuadb_fixture.accounts (
  id integer PRIMARY KEY,
  account_name text NOT NULL UNIQUE
);
CREATE TABLE cuadb_fixture.orders (
  "ID" integer GENERATED ALWAYS AS IDENTITY,
  "Code" varchar(20) NOT NULL,
  account_id integer NOT NULL REFERENCES cuadb_fixture.accounts(id),
  state cuadb_fixture.review_state NOT NULL DEFAULT 'new',
  value cuadb_fixture.nonnegative,
  computed integer GENERATED ALWAYS AS (value + 1) STORED,
  PRIMARY KEY ("ID", "Code")
);
CREATE UNIQUE INDEX orders_code_account_idx
  ON cuadb_fixture.orders ((lower("Code")), account_id)
  WHERE state = 'new';

CREATE TABLE cuadb_fixture.events (
  recorded_on date NOT NULL,
  payload text
) PARTITION BY RANGE (recorded_on);
CREATE TABLE cuadb_fixture.events_2026 PARTITION OF cuadb_fixture.events
  FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');

CREATE VIEW cuadb_fixture.accounts_view AS SELECT id, account_name
  FROM cuadb_fixture.accounts;
CREATE MATERIALIZED VIEW cuadb_fixture.accounts_mat AS
  SELECT id, account_name FROM cuadb_fixture.accounts WITH NO DATA;
CREATE TABLE "CuaDB Mixed"."Ünicode Table" ("A B" text, "Amount" numeric(12,2));

CREATE SEQUENCE cuadb_fixture.sample_sequence;
CREATE FUNCTION cuadb_fixture.read_only_echo(value text)
  RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT value $$;
CREATE PROCEDURE cuadb_fixture.do_nothing()
  LANGUAGE plpgsql AS $$ BEGIN NULL; END $$;
CREATE FUNCTION cuadb_fixture.noop_trigger()
  RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;
CREATE TRIGGER orders_noop BEFORE INSERT ON cuadb_fixture.orders
  FOR EACH ROW EXECUTE FUNCTION cuadb_fixture.noop_trigger();

INSERT INTO cuadb_fixture.accounts(id,account_name)
  VALUES (1,'sample'), (2,'another');
INSERT INTO cuadb_fixture.orders("Code",account_id,value)
  VALUES ('first',1,3), ('second',2,5);
INSERT INTO cuadb_fixture.events VALUES ('2026-05-06','sample');
REFRESH MATERIALIZED VIEW cuadb_fixture.accounts_mat;
ANALYZE cuadb_fixture.accounts;
ANALYZE cuadb_fixture.orders;
ANALYZE cuadb_fixture.events_2026;

-- Before granting, create a dedicated reader role on this disposable server
-- and adjust it to your environment, then run the following as an admin:
-- GRANT CONNECT ON DATABASE cuadb_fixture_catalog TO cuadb_reader;
-- GRANT USAGE ON SCHEMA cuadb_fixture, "CuaDB Mixed" TO cuadb_reader;
-- GRANT SELECT ON ALL TABLES IN SCHEMA cuadb_fixture, "CuaDB Mixed" TO cuadb_reader;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA cuadb_fixture TO cuadb_reader;
-- GRANT USAGE ON TYPE cuadb_fixture.review_state, cuadb_fixture.nonnegative TO cuadb_reader;
