-- Parametric view calling syntax
SELECT * 
FROM param_view(
    param1={param1:String},
    param2={param2:UInt32}
);