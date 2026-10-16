SELECT prior.c1
FROM t AS prior
START WITH prior.c1 = 1
CONNECT BY PRIOR prior.c1 = prior.parent_c1;
