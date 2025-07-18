-- Test just MERGE JOIN without LEFT/RIGHT/INNER
SELECT * FROM TableA a MERGE JOIN TableB b ON a.id = b.id;