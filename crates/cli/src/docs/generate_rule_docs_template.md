# Rules

The following rules are available in this create. This list is generated from the `rules` module in the source code and can be turned on or off and configured in the config file. 

## Rule Index

| Rule Code | Rule Name | 
|-----------|-----------|{% for rule in rules %}
| {{ rule.code }} | [{{ rule.name }}](#{{ rule.name }}) |{% endfor %}

## Rule Details
{% for rule in rules %}
### {{ rule.name }}

{{ rule.description }}

**Code:** {{ rule.code }}

**Fixable:** {% if rule.fixable %}Yes{% else %}No{% endif %}

{% if rule.long_description %}{{ rule.long_description }}{% endif %}
{% endfor %}
