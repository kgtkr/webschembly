(module
  (type $MutCell (sub final (struct (field (mut eqref)))))
  (type $Nil (sub final (struct)))
  (type $Bool (sub final (struct (field i8))))
  (type $Char (sub final (struct (field i32))))
  (type $Int (sub final (struct (field i64))))
  (type $Float (sub final (struct (field f64))))
  (type $Cons (sub final (struct (field $car (mut eqref)) (field $cdr (mut eqref)))))
  ;; Rustとのやり取りの関係でWasmGCの配列ではなくメモリに配置する。TODO: FinalizationRegistryを使ってGC
  (type $StringBuf (sub final (struct (field $ptr i32) (field $shared (mut i8)))))
  (type $String (sub final (struct (field $buf (mut (ref $StringBuf))) (field $len i32) (field $offset i32))))
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
  (import "runtime" "_int_to_string" (func $_int_to_string (param i64) (result i64)))
  (import "runtime" "write_buf_" (func $write_buf_ (param i32) (param i32)))
  (import "env" "_register_string_buf" (func $_register_string_buf (param (ref $StringBuf)) (param i32)))
  (global $nil (export "nil") (ref $Nil) (struct.new $Nil))
  (global $true (export "true") (ref $Bool) (struct.new $Bool (i32.const 1)))
  (global $false (export "false") (ref $Bool) (struct.new $Bool (i32.const 0)))
  (table $globals (export "globals") 1 eqref)
  (table $builtins (export "builtins") 1 eqref)
  (table $symbols 1 (ref null $Symbol))

  (func $display (export "display") (param $s (ref $String))
    (local $s_ptr i32)
    (local $s_len i32)
    (call $string_to_rust (local.get $s)) (local.set $s_ptr) (local.set $s_len)
    (call $write_buf_ (local.get $s_ptr) (local.get $s_len))
  )
  (func $string_to_symbol (export "string_to_symbol") (param $s (ref $String)) (result (ref $Symbol))
    ;; TODO: r5rsのstringは可変らしいのでコピーが必要
    (local $s_ptr i32)
    (local $s_len i32)
    (local $symbol_index i32)
    (local $new_symbol (ref $Symbol))
    
    ;; string -> symbol_index
    (call $string_to_rust (local.get $s)) (local.set $s_ptr) (local.set $s_len)
    (local.set $symbol_index (call $_string_to_symbol (local.get $s_ptr) (local.get $s_len)))

    ;; grow symbol table
    (block $break
      (loop $loop
        (br_if $break
          (i32.gt_u (table.size $symbols) (local.get $symbol_index))
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

  (func $int_to_string (export "int_to_string") (param $x i64) (result (ref $String))
    (local $s_ptr i32)
    (local $s_len i32)
    (local $s (ref $String))
    (call $uncos_tuple_i32 (call $_int_to_string (local.get $x))) (local.set $s_ptr) (local.set $s_len)
    (local.set $s (call $string_from_rust (local.get $s_ptr) (local.get $s_len)))
    (local.get $s)
  )

  ;; i64を(i32, i32)として解釈する
  (func $uncos_tuple_i32 (param $x i64) (result i32) (result i32)
    (i32.wrap_i64 (i64.shr_u (local.get $x) (i64.const 32)))
    (i32.wrap_i64 (local.get $x))
  )

  (func $string_to_rust (param $s (ref $String)) (result i32) (result i32)
    (local $s_buf (ref $StringBuf))
    (local $s_ptr i32)
    (local $s_len i32)

    (local.set $s_buf (struct.get $String $buf (local.get $s)))
    (local.set $s_len (struct.get $String $len (local.get $s)))
    (local.set $s_ptr (i32.add (struct.get $StringBuf $ptr (local.get $s_buf)) (struct.get $String $offset (local.get $s))))

    (local.get $s_len)
    (local.get $s_ptr)
  )

  (func $string_from_rust (param $s_ptr i32) (param $s_len i32) (result (ref $String))
    (local $s_buf (ref $StringBuf))

    (local.set $s_buf (struct.new $StringBuf (local.get $s_ptr) (i32.const 0)))
    (call $register_string_buf (local.get $s_buf)) (drop)
    (struct.new $String (local.get $s_buf) (local.get $s_len) (i32.const 0))
  )

  (func $register_string_buf (export "register_string_buf") (param $s_buf (ref $StringBuf)) (result (ref $StringBuf))
    (call $_register_string_buf (local.get $s_buf) (struct.get $StringBuf $ptr (local.get $s_buf)))
    ;; TODO: コード生成がめんどくさいので一旦引数を返す
    (local.get $s_buf)
  )

  (start $init)
)
