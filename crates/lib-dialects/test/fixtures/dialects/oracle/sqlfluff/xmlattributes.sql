-- XMLELEMENT with nested XMLATTRIBUTES aliases.
select
   xmlelement(
      "xml_el"
      , xmlattributes (
         'attr1' as "attr_name1"
        ,'attr2' as "attr_name2"
      )
   )
from ex_tab;

-- AS is optional and attribute values can be expressions.
select
   xmlelement(
      "rec"
      , xmlattributes(id as "id", first_name || ' ' || last_name full_name)
   )
from employees;

-- Dynamic attribute names use AS EVALNAME followed by an expression.
select
   xmlelement(
      "rec"
      , xmlattributes(val as evalname 'attr_' || col_name)
   )
from ex_tab;
