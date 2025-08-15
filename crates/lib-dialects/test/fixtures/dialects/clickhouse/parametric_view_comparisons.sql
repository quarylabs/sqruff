-- Parametric view with comparison operators
CREATE VIEW comparison_param_view AS
SELECT id, timestamp, value
FROM events
WHERE timestamp >= {start_date:DateTime64}
  AND timestamp <= {end_date:DateTime64}
  AND value > {min_value:Float64}
  AND status != {excluded_status:String}
  AND score < {max_score:UInt32};