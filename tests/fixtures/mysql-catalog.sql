-- Disposable MySQL 8 fixture only. Provision a database named cuadb_fixture_mysql,
-- run as its owner, then grant the viewer SELECT and SHOW VIEW on this database.
CREATE TABLE IF NOT EXISTS `Mixed_Items` (
  id INT PRIMARY KEY AUTO_INCREMENT,
  label VARCHAR(40) NOT NULL,
  payload BLOB,
  amount DECIMAL(8,2),
  enabled TINYINT(1) NOT NULL DEFAULT 1,
  generated_label VARCHAR(40) GENERATED ALWAYS AS (UPPER(label)) STORED
) ENGINE=InnoDB;
CREATE TABLE IF NOT EXISTS child (
  id INT PRIMARY KEY,
  item_id INT,
  CONSTRAINT fk_item FOREIGN KEY (item_id) REFERENCES `Mixed_Items`(id)
) ENGINE=InnoDB;
CREATE OR REPLACE VIEW items_view AS SELECT id,label FROM `Mixed_Items`;
INSERT INTO `Mixed_Items` (label,payload,amount) VALUES ('fixture',0x00FF,12.34);
