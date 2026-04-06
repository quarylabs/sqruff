CREATE OR REPLACE PROCEDURE `project.dataset.my_proc` (
  p_param INT64
)
OPTIONS (
  description = "Test procedure",
  strict_mode = TRUE
)
BEGIN
  SELECT p_param;
END;
