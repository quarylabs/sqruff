-- Test only INNER HASH JOIN which we know works
SELECT * FROM TableA a INNER HASH JOIN TableB b ON a.id = b.id;