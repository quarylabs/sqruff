# Dialects

Sqruff currently supports the following SQL dialects:

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
{{ dialect.config_section }}
```
{% endfor %}
We are working on adding support for more dialects in the future.
