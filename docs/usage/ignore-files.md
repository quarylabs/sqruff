# Ignore files

Sqruff ignores files and folders specified in a `.sqruffignore` file placed in the root of where the command is run.
For example, the following config will ignore `.hql` files and files in any directory named temp:

```
# ignore ALL .hql files
*.hql

# ignore ALL files in ANY directory named temp
temp/
```
