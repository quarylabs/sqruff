UPDATE tt
SET tt.rn += 1
FROM table1 AS tt
JOIN src ON tt._id = src._id;