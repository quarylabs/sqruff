# Templaters

Templaters allow you to format/lint non-standard SQL code with sqruff that is transformed in some application setting.
An example of this may be SQL code that is templated with dynamic parameters, for example the below `:id` is replaced at
runtime.

```sql
SELECT id, name
FROM users
WHERE id = :id
```

The templater is set in the config file as follows:

```ini
[sqruff]
templater = raw
```

## Templaters Index

Sqruff comes with the following templaters out of the box:

- [raw](raw)
- [placeholder](placeholder)

## Details

### raw

The raw templater simply returns the input string as the output string. It passes through the input string unchanged and is useful if you need no templating. It is the defualt templater.

### placeholder

Libraries such as SQLAlchemy or Psycopg use different parameter placeholder styles to mark where a parameter has to be inserted in the query.

For example a query in SQLAlchemy can look like this:

```sql
SELECT * FROM table WHERE id = :myid
```

At runtime :myid will be replace by a value provided by the application and escaped as needed, but this is not standard SQL and cannot be parsed as is.

In order to parse these queries is then necessary to replace these placeholders with sample values, and this is done with the placeholder templater.

Placeholder templating can be enabled in the config using:

```ini
[sqruff]
templater = placeholder
```

A few common styles are supported:

```sql
 -- colon
 WHERE bla = :my_name

 -- colon_nospaces
 -- (use with caution as more prone to false positives)
 WHERE bla = table:my_name

 -- colon_optional_quotes
 SELECT :"column" FROM :table WHERE bla = :'my_name'

 -- numeric_colon
 WHERE bla = :2

 -- pyformat
 WHERE bla = %(my_name)s

 -- dollar
 WHERE bla = $my_name or WHERE bla = ${my_name}

 -- question_mark
 WHERE bla = ?

 -- numeric_dollar
 WHERE bla = $3 or WHERE bla = ${3}

 -- percent
 WHERE bla = %s

 -- ampersand
 WHERE bla = &s or WHERE bla = &{s} or USE DATABASE MARK_{ENV}
```

The can be configured by setting `param_style` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_style = colon
my_name = 'john'
```

then you can set sample values for each parameter, like my_name above. Notice that the value needs to be escaped as it will be replaced as a string during parsing. When the sample values aren’t provided, the templater will use parameter names themselves by default.

When parameters are positional, like question_mark, then their name is simply the order in which they appear, starting with 1.

```ini
[sqruff:templater:placeholder]
param_style = question_mark
1 = 'john'
```

In case you nbeed a parameter style different from the ones provided, you can set `param_regex` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_regex = __(?P<param_name>[\w_]+)__
my_name = 'john'
```

N.B. quotes around param_regex in the config are interpreted literally by the templater. e.g. param_regex=’__(?P<param_name>[w_]+)__’ matches ‘__some_param__’ not __some_param__

the named parameter param_name will be used as the key to replace, if missing, the parameter is assumed to be positional and numbers are used instead.

Also consider making a pull request to the project to have your style added, it may be useful to other people and simplify your configuration.
