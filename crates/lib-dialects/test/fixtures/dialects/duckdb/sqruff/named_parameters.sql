SELECT $name, $_value, $value1, $MixedCase, $名前;

SELECT $name::VARCHAR, CAST($count AS INTEGER), COALESCE($name, 'unknown');

SELECT * FROM items WHERE ($start IS NULL OR id >= $start) AND id <= $end;

SELECT CASE WHEN $target == 'stream_id' THEN stream_id ELSE name END FROM items;

SELECT $1, $2;

SELECT '$name', "$name", $$a $name string$$, $tag$a $name string$tag$;
