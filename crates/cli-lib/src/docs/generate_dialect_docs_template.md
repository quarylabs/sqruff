# Dialects

Sqruff currently supports the following SQL dialects.

## Configuration

To configure a dialect, set the `dialect` option in your `.sqruff` configuration file:

```ini
[sqruff]
dialect = <dialect_name>
```

For example, to use Snowflake:

```ini
[sqruff]
dialect = snowflake
```

You can also specify the dialect on the command line:

```bash
sqruff lint --dialect snowflake myfile.sql
```

## Dialects Index
{% for dialect in dialects %}
- [{{ dialect.name }}](#{{ dialect.name }})
{%- endfor %}

## Details
{% for dialect in dialects %}
### {{ dialect.name }}

{{ dialect.description }}
{% if dialect.doc_url %}
**Documentation:** [{{ dialect.doc_url }}]({{ dialect.doc_url }})
{% endif %}
**Configuration:**
```ini
[sqruff]
dialect = {{ dialect.name }}
```
{% endfor %}
We are working on adding support for more dialects in the future.
