LOCK TABLE films IN SHARE MODE;
LOCK TABLE films IN SHARE ROW EXCLUSIVE MODE;
LOCK TABLE team IN ACCESS EXCLUSIVE MODE;
lock table stud1 IN SHARE UPDATE EXCLUSIVE MODE;
LOCK TABLE crontable NOWAIT;