-- Parametric view creation and calling
CREATE VIEW user_activity AS
SELECT user_id, event_type, count() as event_count
FROM events
WHERE date >= {start_date:Date}
  AND date <= {end_date:Date}
  AND status = {status:String}
GROUP BY user_id, event_type;

-- Calling parametric view with parameters
SELECT * 
FROM user_activity(
    start_date={start:Date},
    end_date={end:Date},
    status={active:String}
);