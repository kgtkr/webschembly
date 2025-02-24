(module
  (type $MutCell (sub final (struct (field (mut eqref)))))
  (type $Nil (sub final (struct)))
  (type $Bool (sub final (struct (field i8))))
  (type $Char (sub final (struct (field i32))))
  (type $Int (sub final (struct (field i64))))
  (type $Float (sub final (struct (field f64))))
  (type $Cons (sub final (struct (field $car (mut eqref)) (field $cdr (mut eqref)))))
  (type $String (array (field i8)))
  (type $Symbol (sub final (struct (field $name (ref $String)))))
  (type $Vector (array (mut eqref)))
  (type $VariableParams (array (field eqref)))
  (rec
      (type $BoxedFunc (func (param (ref $Closure)) (param (ref $VariableParams)) (result eqref)))
      (type $Closure (sub (struct
          (field $func (ref func))
          (field $boxed_func (ref $BoxedFunc)))))
  )
  (import "runtime" "init" (func $init))
  (global $nil (export "nil") (ref $Nil) (struct.new $Nil))
  (global $true (export "true") (ref $Bool) (struct.new $Bool (i32.const 1)))
  (global $false (export "false") (ref $Bool) (struct.new $Bool (i32.const 0)))
  (table $globals (export "globals") 1 eqref)
  (table $builtins (export "builtins") 1 eqref)


  (func $display (export "display") (param $value eqref))
  (func $string_to_symbol (export "string_to_symbol") (param $s (ref $String)) (result (ref $Symbol))
    (struct.new $Symbol (local.get $s)) ;; TODO:
  )
  (start $init)
)
