# Dialects

Sqruff currently supports the following SQL dialects:

Each dialect includes a collapsed list of segment types generated from its
reachable grammar and lexer using the default dialect configuration. Use these
names in `[sqruff:layout:type:<type>]` configuration section headings. Some types
are internal or meta segments; run `sqruff parse query.sql --dialect <dialect>`
to inspect the concrete syntax tree for a particular query.

## Dialects Index
{% for dialect in dialects %}
- [{{ dialect.name }}](#{{ dialect.name }})
{%- endfor %}

## Details
{% for dialect in dialects %}
### {{ dialect.name }}

{{ dialect.description }}{% if dialect.default_casing %}

**Default Casing:** {{ dialect.default_casing }}{% endif %}{% if dialect.quotes %}

**Quotes:** {{ dialect.quotes }}{% endif %}
{% if dialect.doc_url %}
**Documentation:** [{{ dialect.doc_url }}]({{ dialect.doc_url }})
{% endif %}
**Configuration:**
```ini
{{ dialect.config_section }}
```
{% if dialect.has_config_options %}
**Options:**

| Option | Description | Default |
|--------|-------------|---------|
{%- for option in dialect.config_options %}
| `{{ option.name }}` | {{ option.description }} | `{{ option.default }}` |
{%- endfor %}

**Example:**
```ini
{{ dialect.config_section }}
{%- for option in dialect.config_options %}
{{ option.name }} = true
{%- endfor %}
```
{% endif %}
<details>
<summary>Segment types ({{ dialect.segment_types | length }})</summary>
<ul>
{% for segment_type in dialect.segment_types -%}
<li><code>{{ segment_type }}</code></li>
{% endfor -%}
</ul>
</details>

{% endfor %}
We are working on adding support for more dialects in the future.
