-- Does the key exist?
SELECT '{"a":1, "b":2}'::jsonb ? 'b';
-- Do any of the keys exist?
SELECT '{"a":1, "b":2, "c":3}'::jsonb ?| ARRAY['b', 'd'];
-- Do all of the keys exist?
SELECT '["a", "b", "c"]'::jsonb ?& ARRAY['a', 'b'];
-- Does the JSON path return any item?
SELECT '{"a":[1,2,3,4,5]}'::jsonb @? '$.a[*] ? (@ > 2)';
-- Does the JSON path predicate hold?
SELECT '{"a":[1,2,3,4,5]}'::jsonb @@ '$.a[*] > 2';
-- Delete the value at the path
SELECT '["a", {"b":1}]'::jsonb #- '{1,b}';
-- The whole family in a WHERE clause
SELECT *
FROM mytable
WHERE
  doc ? 'a'
  AND doc ?| ARRAY['b']
  AND doc ?& ARRAY['c']
  AND doc @? '$.d'
  AND doc @@ '$.e == 1';
