CREATE TABLE `foo` (
  b VARCHAR(255) BINARY,
  `id` int(11) unsigned NOT NULL AUTO_INCREMENT,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

create table _ (a int);

CREATE TABLE foo SELECT * FROM bar;

CREATE TEMPORARY TABLE tbl_name (
    id INT PRIMARY KEY AUTO_INCREMENT,
    col VARCHAR(255) DEFAULT '' NOT NULL,
    INDEX(col)
) AS SELECT id, col FROM table_name;

CREATE TEMPORARY TABLE tbl_name (
    id INT PRIMARY KEY AUTO_INCREMENT,
    col VARCHAR(255) DEFAULT '' NOT NULL,
    INDEX(col)
) SELECT id, col FROM table_name;

CREATE TEMPORARY TABLE tbl_name (INDEX(col)) AS
    SELECT id, col FROM table_name;

CREATE TEMPORARY TABLE tbl_name (INDEX(col))
    SELECT id, col FROM table_name;
