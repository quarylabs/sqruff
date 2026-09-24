# Ignore files

Sqruff ignores files and folders specified in a `.sqruffignore` file placed in the root of where the command is run.
For example, the following config will ignore `.hql` files and files in any directory named temp:

```
# ignore ALL .hql files
*.hql

# ignore ALL files in ANY directory named temp
temp/
```

## `pyproject.toml`

If you use `pyproject.toml` for Sqruff configuration, you can also ignore files
and directories using `ignore_paths` in `[tool.sqlfluff.core]`.

```toml
[tool.sqlfluff.core]
ignore_paths = [
    "target/",
    "supabase/migrations/*",
    "generated/*.sql",
]
```

The patterns in `ignore_paths` use the same matching rules as `.sqruffignore`.
