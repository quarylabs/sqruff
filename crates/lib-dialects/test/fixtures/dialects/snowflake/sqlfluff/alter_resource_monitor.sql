-- Test cases for ALTER RESOURCE MONITOR statement
-- From SQLFluff PR #5272

-- Simple alter with credit quota
ALTER RESOURCE MONITOR my_monitor SET CREDIT_QUOTA=200;

-- Alter with frequency
ALTER RESOURCE MONITOR budget_monitor SET FREQUENCY=WEEKLY;

-- Alter with multiple options
ALTER RESOURCE MONITOR my_monitor SET
    CREDIT_QUOTA=300
    FREQUENCY=MONTHLY;

-- Alter with start timestamp
ALTER RESOURCE MONITOR timed_monitor SET START_TIMESTAMP='2024-06-01 00:00:00';
ALTER RESOURCE MONITOR immediate_monitor SET START_TIMESTAMP=IMMEDIATELY;

-- Alter with end timestamp
ALTER RESOURCE MONITOR limited_monitor SET END_TIMESTAMP='2025-12-31 23:59:59';

-- Alter with notify users
ALTER RESOURCE MONITOR notify_monitor SET NOTIFY_USERS=(user1, user2, admin);

-- Alter with triggers
ALTER RESOURCE MONITOR trigger_monitor SET
    TRIGGERS ON 85 PERCENT DO NOTIFY
             ON 95 PERCENT DO SUSPEND;

-- Alter with all options
ALTER RESOURCE MONITOR full_monitor SET
    CREDIT_QUOTA=3000
    FREQUENCY=YEARLY
    START_TIMESTAMP='2024-02-01 00:00:00'
    END_TIMESTAMP='2025-01-31 23:59:59'
    NOTIFY_USERS=(superadmin, billing_team)
    TRIGGERS ON 70 PERCENT DO NOTIFY
             ON 85 PERCENT DO SUSPEND
             ON 98 PERCENT DO SUSPEND_IMMEDIATE;

-- Alter with IF EXISTS
ALTER RESOURCE MONITOR IF EXISTS my_monitor SET CREDIT_QUOTA=150;

-- Alter with schema qualified name
ALTER RESOURCE MONITOR my_schema.my_monitor SET CREDIT_QUOTA=250;

-- Alter with database and schema qualified name
ALTER RESOURCE MONITOR my_db.my_schema.my_monitor SET CREDIT_QUOTA=350;

-- Alter with different frequency options
ALTER RESOURCE MONITOR monitor1 SET FREQUENCY=DAILY;
ALTER RESOURCE MONITOR monitor2 SET FREQUENCY=WEEKLY;
ALTER RESOURCE MONITOR monitor3 SET FREQUENCY=MONTHLY;
ALTER RESOURCE MONITOR monitor4 SET FREQUENCY=YEARLY;
ALTER RESOURCE MONITOR monitor5 SET FREQUENCY=NEVER;