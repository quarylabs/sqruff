-- Test cases for CREATE RESOURCE MONITOR statement
-- From SQLFluff PR #5272

-- Simple resource monitor with credit quota
CREATE RESOURCE MONITOR my_monitor WITH CREDIT_QUOTA=100;

-- Resource monitor with frequency
CREATE RESOURCE MONITOR budget_monitor WITH CREDIT_QUOTA=500 FREQUENCY=MONTHLY;

-- Resource monitor with all frequency options
CREATE RESOURCE MONITOR daily_monitor WITH CREDIT_QUOTA=10 FREQUENCY=DAILY;
CREATE RESOURCE MONITOR weekly_monitor WITH CREDIT_QUOTA=70 FREQUENCY=WEEKLY;
CREATE RESOURCE MONITOR yearly_monitor WITH CREDIT_QUOTA=5000 FREQUENCY=YEARLY;
CREATE RESOURCE MONITOR never_monitor WITH CREDIT_QUOTA=1000 FREQUENCY=NEVER;

-- Resource monitor with start timestamp
CREATE RESOURCE MONITOR timed_monitor WITH CREDIT_QUOTA=100 START_TIMESTAMP='2024-01-01 00:00:00';
CREATE RESOURCE MONITOR immediate_monitor WITH CREDIT_QUOTA=100 START_TIMESTAMP=IMMEDIATELY;

-- Resource monitor with end timestamp
CREATE RESOURCE MONITOR limited_monitor WITH CREDIT_QUOTA=100 END_TIMESTAMP='2024-12-31 23:59:59';

-- Resource monitor with notify users
CREATE RESOURCE MONITOR notify_monitor WITH CREDIT_QUOTA=100 NOTIFY_USERS=(user1, user2, user3);

-- Resource monitor with single trigger
CREATE RESOURCE MONITOR trigger_monitor WITH CREDIT_QUOTA=100
    TRIGGERS ON 90 PERCENT DO SUSPEND;

-- Resource monitor with multiple triggers
CREATE RESOURCE MONITOR multi_trigger_monitor WITH CREDIT_QUOTA=1000
    TRIGGERS ON 50 PERCENT DO NOTIFY
             ON 75 PERCENT DO NOTIFY
             ON 90 PERCENT DO SUSPEND
             ON 100 PERCENT DO SUSPEND_IMMEDIATE;

-- Resource monitor with all options
CREATE OR REPLACE RESOURCE MONITOR full_monitor WITH
    CREDIT_QUOTA=2000
    FREQUENCY=MONTHLY
    START_TIMESTAMP='2024-01-01 00:00:00'
    END_TIMESTAMP='2024-12-31 23:59:59'
    NOTIFY_USERS=(admin, finance_team)
    TRIGGERS ON 60 PERCENT DO NOTIFY
             ON 80 PERCENT DO SUSPEND
             ON 95 PERCENT DO SUSPEND_IMMEDIATE;

-- Resource monitor with OR REPLACE
CREATE OR REPLACE RESOURCE MONITOR replaced_monitor WITH CREDIT_QUOTA=300;

-- Resource monitor with schema qualified name
CREATE RESOURCE MONITOR my_schema.my_monitor WITH CREDIT_QUOTA=100;

-- Resource monitor with database and schema qualified name
CREATE RESOURCE MONITOR my_db.my_schema.my_monitor WITH CREDIT_QUOTA=100;