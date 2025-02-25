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
  (import "runtime" "malloc" (func $malloc (param i32) (result i32)))
  (import "runtime" "free" (func $free (param i32)))
  (import "runtime" "memory" (memory 1))
  (import "runtime" "_string_to_symbol" (func $_string_to_symbol (param i32) (param i32) (result i32)))
  (global $nil (export "nil") (ref $Nil) (struct.new $Nil))
  (global $true (export "true") (ref $Bool) (struct.new $Bool (i32.const 1)))
  (global $false (export "false") (ref $Bool) (struct.new $Bool (i32.const 0)))
  (table $globals (export "globals") 1 eqref)
  (table $builtins (export "builtins") 1 eqref)
  (table $symbols 1 (ref null $Symbol))

  (func $display (export "display") (param $value eqref))
  (func $string_to_symbol (export "string_to_symbol") (param $s (ref $String)) (result (ref $Symbol))
    ;; TODO: r5rsのstringは可変らしいのでコピーが必要
    (local $s_ptr i32)
    (local $s_len i32)
    (local $symbol_index i32)
    (local $new_symbol (ref $Symbol))
    (local $i i32)
    
    ;; string -> symbol_index
    (local.set $s_len (array.len (local.get $s)))
    (local.set $s_ptr (call $malloc (local.get $s_len)))
    ;; array copy to memory
    ;; 今のところループを回すしかなさそう: https://github.com/WebAssembly/gc/issues/395
    (local.set $i (i32.const 0))
    (block $break
      (loop $loop
        (br_if $break
          (i32.ge_u (local.get $i) (local.get $s_len))
        )
        (i32.store8 (i32.add (local.get $s_ptr) (local.get $i)) (array.get_u $String (local.get $s) (local.get $i)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )
    (local.set $symbol_index (call $_string_to_symbol (local.get $s_ptr) (local.get $s_len)))
    (call $free (local.get $s_ptr))

    ;; grow symbol table
    (block $break
      (loop $loop
        (br_if $break
          (i32.ge_u (table.size $symbols) (local.get $symbol_index))
        )
        (if (i32.eq (i32.const -1) (table.grow $symbols (ref.null $Symbol) (table.size $symbols)))
          (then
            (unreachable)
          )
        )
        (br $loop)
      )
    )

    (return (block $exist (result (ref $Symbol))
      (br_on_non_null $exist (table.get $symbols (local.get $symbol_index)))
      (local.set $new_symbol (struct.new $Symbol (local.get $s)))
      (table.set $symbols (local.get $symbol_index) (local.get $new_symbol))
      (local.get $new_symbol)
    ))
  )

  (start $init)
)
