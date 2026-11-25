SELECT substring(name FROM 2 FOR length(name) - 1) FROM tbl;
SELECT substring(name FROM 2 FOR 3) FROM tbl;
SELECT substring(name FOR 3) FROM tbl;
SELECT substring(name FROM 2) FROM tbl;
