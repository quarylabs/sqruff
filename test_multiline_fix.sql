-- Test multiline T-SQL alias with TOP
SELECT TOP 20
    JiraIssueID = JiraIssue.i_jira_id
FROM JiraIssue;