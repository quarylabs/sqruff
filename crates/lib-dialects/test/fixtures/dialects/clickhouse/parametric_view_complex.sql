-- Parametric view with complex types
CREATE VIEW complex_param_view AS
SELECT id, value
FROM table2
WHERE type = {param1:Enum('type1', 'type2')}
  AND date_col = {param2:Nullable(DateTime64)};