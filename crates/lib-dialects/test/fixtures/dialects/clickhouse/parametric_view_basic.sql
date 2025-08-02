-- Basic parametric view with simple types
CREATE VIEW param_view AS
SELECT id, name
FROM table1
WHERE status = {param1:String}
  AND count > {param2:UInt32};