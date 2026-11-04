-- Additive, idempotent fixture for MySQL 8 and MariaDB 11.8. Run as the fixture
-- operator inside the cuadb_fixture_mysql / cuadb_fixture_mariadb database ONLY.
-- Never drops anything; rows are inserted only into empty tables.
CREATE TABLE IF NOT EXISTS rich_values (
  id INT PRIMARY KEY AUTO_INCREMENT,
  doc JSON,
  small_bin VARBINARY(16),
  big_blob LONGBLOB,
  price DECIMAL(30,10),
  at DATETIME(6),
  d DATE,
  tt TIME(3),
  txt LONGTEXT,
  emoji VARCHAR(40) CHARACTER SET utf8mb4,
  flag TINYINT(1),
  mode ENUM('a','b'),
  tags SET('x','y'),
  bits BIT(8),
  nullable_text VARCHAR(20)
) ENGINE=InnoDB;
INSERT INTO rich_values (doc, small_bin, big_blob, price, at, d, tt, txt, emoji, flag, mode, tags, bits, nullable_text)
SELECT '{"a":{"b":[1,2,{"c":null}]},"s":"<script>alert(1)</script>"}', 0x00FF10, REPEAT(0xAB, 100000),
       12345678901234567890.1234567890, '2026-01-02 03:04:05.123456', '2026-01-02', '12:34:56.789',
       CONCAT('<script>alert(1)</script>', CHAR(7), ' line', CHAR(10), 'break'),
       '😀 日本 Ünï', 1, 'b', 'x,y', b'10100101', 'first'
WHERE (SELECT COUNT(*) FROM rich_values) = 0;
INSERT INTO rich_values (txt, nullable_text)
SELECT REPEAT('x', 100000), NULL
WHERE (SELECT COUNT(*) FROM rich_values) = 1;
INSERT INTO rich_values (nullable_text)
SELECT CONCAT('row ', n) FROM (
  WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 40) SELECT n FROM seq
) s
WHERE (SELECT COUNT(*) FROM rich_values) = 2;
CREATE TABLE IF NOT EXISTS empty_t (id INT PRIMARY KEY, note VARCHAR(10)) ENGINE=InnoDB;
CREATE TABLE IF NOT EXISTS legacy_myisam (id INT PRIMARY KEY, note VARCHAR(10)) ENGINE=MyISAM;
INSERT INTO legacy_myisam SELECT 1, 'legacy' WHERE (SELECT COUNT(*) FROM legacy_myisam) = 0;
