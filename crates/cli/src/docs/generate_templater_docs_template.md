# Templaters

Templaters allow you to format/lint non-standard SQL code with sqruff that is transformed in some application setting.
An example of this may be SQL code that is templated with dynamic parameters, for example the below `:id` is replaced at
runtime.

```sql
SELECT id, name
FROM users
WHERE id = :id
```

## Templaters Index

Sqruff comes with the following templaters out of the box:

{% for template in templaters %}- [{{ template.name }}]({{ template.name }})
{% endfor %}
## Details
{% for templater in templaters %}
### {{ templater.name }}

{{ templater.description }}
{% endfor %}