WITH bu2ic AS (
    SELECT ItemCategoryID FROM table1
)
SELECT *
FROM NxItem
LEFT OUTER JOIN bu2ic AS BU2IC ON BU2IC.ItemCategoryID = NxItem.i_category_id