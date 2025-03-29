(module
  (type $MutCell (sub final (struct (field (mut eqref)))))
  (type $Nil (sub final (struct)))
  (type $Bool (sub final (struct (field i8))))
  (type $Char (sub final (struct (field i32))))
  (type $Int (sub final (struct (field i64))))
  (type $Float (sub final (struct (field f64))))
  (type $Cons (sub final (struct (field $car (mut eqref)) (field $cdr (mut eqref)))))
  (type $Buf (array (mut i8)))
  (type $StringBuf (sub final (struct (field $buf (ref $Buf)) (field $shared (mut i8)))))
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
  ;; TODO: いらなくなったら削除
  (type $Func1 (func (param (ref $Closure)) (param eqref) (result eqref)))
  ;;
  (import "runtime" "init" (func $init))
  (import "runtime" "malloc" (func $malloc (param i32) (result i32)))
  (import "runtime" "free" (func $free (param i32)))
  (import "runtime" "memory" (memory 1))
  (import "runtime" "_string_to_symbol" (func $_string_to_symbol (param i32) (param i32) (result i32)))
  (import "runtime" "_int_to_string" (func $_int_to_string (param i64) (result i64)))
  (import "runtime" "write_buf" (func $write_buf (param i32) (param i32) (param i32)))
  (import "runtime" "write_char" (func $write_char (param i32)))
  (global $nil (export "nil") (ref $Nil) (struct.new $Nil))
  (global $true (export "true") (ref $Bool) (struct.new $Bool (i32.const 1)))
  (global $false (export "false") (ref $Bool) (struct.new $Bool (i32.const 0)))
  (table $globals (export "globals") 1 eqref)
  (table $symbols 1 (ref null $Symbol))
  (tag $WEBSCHEMBLY_EXCEPTION (export "WEBSCHEMBLY_EXCEPTION"))

  (func $display_fd (export "display_fd") (param $fd i32) (param $s (ref $String))
    (local $s_ptr i32)
    (local $s_len i32)
    (call $string_to_memory (local.get $s)) (local.set $s_ptr) (local.set $s_len)
    (call $write_buf (local.get $fd) (local.get $s_ptr) (local.get $s_len))
    (call $free (local.get $s_ptr))
  )
  (func $display (export "display") (param $s (ref $String))
    (call $display_fd (i32.const 1) (local.get $s))
  )
  (func $string_to_symbol (export "string_to_symbol") (param $s (ref $String)) (result (ref $Symbol))
    (local $s_ptr i32)
    (local $s_len i32)
    (local $symbol_index i32)
    (local $new_symbol (ref $Symbol))

    (local.set $s (call $copy_string (local.get $s)))
    
    ;; string -> symbol_index
    (call $string_to_memory (local.get $s)) (local.set $s_ptr) (local.set $s_len)
    (local.set $symbol_index (call $_string_to_symbol (local.get $s_ptr) (local.get $s_len)))
    (call $free (local.get $s_ptr))

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
    (local.set $s (call $memory_to_string (local.get $s_ptr) (local.get $s_len)))
    (call $free (local.get $s_ptr))
    (local.get $s)
  )

  ;; i64を(i32, i32)として解釈する
  (func $uncos_tuple_i32 (param $x i64) (result i32) (result i32)
    (i32.wrap_i64 (i64.shr_u (local.get $x) (i64.const 32)))
    (i32.wrap_i64 (local.get $x))
  )

  (func $string_to_memory (param $s (ref $String)) (result i32) (result i32)
    (local $ptr i32)
    (local $s_buf (ref $StringBuf))
    (local $len i32)
    

    (local.set $s_buf (struct.get $String $buf (local.get $s)))
    (local.set $len (struct.get $String $len (local.get $s)))

    (local.set $ptr (call $buf_to_memory (struct.get $StringBuf $buf (local.get $s_buf)) (local.get $len) (struct.get $String $offset (local.get $s))))

    (local.get $len)
    (local.get $ptr)
  )

  (func $memory_to_string (param $ptr i32) (param $len i32) (result (ref $String))
    (local $buf (ref $Buf))
    (local $s_buf (ref $StringBuf))
    
    (local.set $buf (call $memory_to_buf (local.get $ptr) (local.get $len)))
    (local.set $s_buf (struct.new $StringBuf (local.get $buf) (i32.const 0)))
    (struct.new $String (local.get $s_buf) (local.get $len) (i32.const 0))
  )

  (func $buf_to_memory (param $buf (ref $Buf)) (param $len i32) (param $offset i32) (result i32)
    (local $ptr i32)
    (local $i i32)

    (local.set $ptr (call $malloc (local.get $len)))

    ;; array copy to memory
    ;; 今のところループを回すしかなさそう: https://github.com/WebAssembly/gc/issues/395
    (block $break
      (loop $loop
        (br_if $break
          (i32.ge_u (local.get $i) (local.get $len))
        )
        (i32.store8 (i32.add (local.get $ptr) (local.get $i)) (array.get_u $Buf (local.get $buf) (i32.add (local.get $offset) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )

    (local.get $ptr)
  )

  (func $memory_to_buf (param $ptr i32) (param $len i32) (result (ref $Buf))
    (local $i i32)
    (local $buf (ref $Buf))

    (local.set $buf (array.new $Buf (i32.const 0) (local.get $len)))
    (block $break
      (loop $loop
        (br_if $break
          (i32.ge_u (local.get $i) (local.get $len))
        )
        (array.set $Buf (local.get $buf) (local.get $i) (i32.load8_u (i32.add (local.get $ptr) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )

    (local.get $buf)
  )

  (func $copy_string (export "copy_string") (param $s (ref $String)) (result (ref $String))
    (local $s_buf (ref $StringBuf))
    (local.set $s_buf (struct.get $String $buf (local.get $s)))
    (struct.set $StringBuf $shared (local.get $s_buf) (i32.const 1))

    (return (struct.new $String (local.get $s_buf) (struct.get $String $len (local.get $s)) (struct.get $String $offset (local.get $s))))
  )

  (func $throw_webassembly_exception (export "throw_webassembly_exception")
    (throw $WEBSCHEMBLY_EXCEPTION)
  )

  (start $init)

  (func $print_for_repl (export "print_for_repl") (param $x eqref)
    ;; TODO: もっと汎用的な方法でJSからschemeのグローバル変数を参照できるようにする
    (local $write_closure (ref $Closure))
    (local $write (ref $Func1))
    ;; writeは今のところ22番目に入っているはず(壊れやすいコードなので要修正)
    (local.set $write_closure (ref.cast (ref $Closure) (table.get $globals (i32.const 22))))
    (local.set $write (ref.cast (ref $Func1) (struct.get $Closure $func (local.get $write_closure))))
    (call_ref $Func1 (local.get $write_closure) (local.get $x) (local.get $write))
    (drop)
    (call $write_char (i32.const 10))
  )
)
