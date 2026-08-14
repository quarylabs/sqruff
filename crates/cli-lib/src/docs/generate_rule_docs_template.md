# Rules

The following rules are available in this create. This list is generated from the `rules` module in the source code and can be turned on or off and configured in the config file. 

## Rule Index

| Rule Code | Rule Name | Description |
|-----------|-----------|-------------|{% for rule in rules %}
| {{ rule.code }} | [{{ rule.name }}](#{{ rule.name_no_periods }}) | {{ rule.description }} | {% endfor %}

## Rule Details
{% for rule in rules %}
### {{ rule.name }}

{{ rule.description }}

**Code:** `{{ rule.code }}`

**Groups:** {% for group in rule.groups %}`{{ group }}`{% if not loop.last %}, {%endif %}{% endfor %}

**Fixable:** {% if rule.fixable %}Yes{% else %}No{% endif %}
{{ rule.long_description }}
{% if rule.has_dialects %}**Dialects where this rule is skipped:** {% for dialect in rule.dialects %}`{{ dialect }}`{% if not loop.last %}, {%endif %}{% endfor %}
{% endif %}{% if rule.has_config_options %}**Configuration:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
{%- for option in rule.config_options %}
| `{{ option.name }}` | {% if option.has_allowed_values %}{% for value in option.allowed_values %}`{{ value }}`{% if not loop.last %} \| {% endif %}{% endfor %}{% else %}{{ option.type_name }}{% endif %} | `{{ option.default }}` | {{ option.description }} |
{%- endfor %}

Set these under `{{ rule.config_section }}` in your config file.
{% endif %}{% endfor %}
