(module
  (type $Locals (struct
    (field $n (mut i32))
    (field $i (mut i32))
    (field $j (mut i32))
    (field $is_prime (mut i32))
    (field $count (mut i32))
  ))

  (func $bb_entry (param $locals (ref $Locals)) (result i32)
    (struct.set $Locals $count (local.get $locals) (i32.const 0))
    (struct.set $Locals $i (local.get $locals) (i32.const 2))

    (return_call $bb_loop1_cond (local.get $locals))
  )
  
  (func $bb_loop1_cond (param $locals (ref $Locals)) (result i32)
    (i32.le_s (struct.get $Locals $i (local.get $locals)) (struct.get $Locals $n (local.get $locals)))
    if
      (return_call $bb_loop1_body (local.get $locals))
    else
      (return (struct.get $Locals $count (local.get $locals)))
    end
  )

  (func $bb_loop1_body (param $locals (ref $Locals)) (result i32)
    (struct.set $Locals $is_prime (local.get $locals) (i32.const 1))
    (struct.set $Locals $j (local.get $locals) (i32.const 2))
    (return_call $bb_loop2_cond (local.get $locals))
  )
  
  (func $bb_loop1_body2 (param $locals (ref $Locals)) (result i32)
    (struct.get $Locals $is_prime (local.get $locals))
    if
      (return_call $bb_if1_then (local.get $locals))
    else
      (return_call $bb_loop1_body3 (local.get $locals))
    end
  )

  (func $bb_if1_then (param $locals (ref $Locals)) (result i32)
    (struct.set $Locals $count (local.get $locals) (i32.add (struct.get $Locals $count (local.get $locals)) (i32.const 1)))
    (return_call $bb_loop1_body3 (local.get $locals))
  )

  (func $bb_loop1_body3 (param $locals (ref $Locals)) (result i32)
    (struct.set $Locals $i (local.get $locals) (i32.add (struct.get $Locals $i (local.get $locals)) (i32.const 1)))
    (return_call $bb_loop1_cond (local.get $locals))
  )

  (func $bb_loop2_cond (param $locals (ref $Locals)) (result i32)
    (i32.lt_s (struct.get $Locals $j (local.get $locals)) (struct.get $Locals $i (local.get $locals)))
    if
      (return_call $bb_loop2_body (local.get $locals))
    else
      (return_call $bb_loop1_body2 (local.get $locals))
    end
  )

  (func $bb_loop2_body (param $locals (ref $Locals)) (result i32)
    (struct.set $Locals $is_prime
      (local.get $locals)
      (i32.and
        (struct.get $Locals $is_prime (local.get $locals))
        (i32.ne
          (i32.rem_u (struct.get $Locals $i (local.get $locals)) (struct.get $Locals $j (local.get $locals)))
          (i32.const 0)
        )
      )
    )

    (struct.set $Locals $j (local.get $locals) (i32.add (struct.get $Locals $j (local.get $locals)) (i32.const 1)))
    (return_call $bb_loop2_cond (local.get $locals))
  )

  (func $prime_count (param $n i32) (result i32)
    (return_call $bb_entry (struct.new $Locals (local.get $n) (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0)))
  )

  (export "prime_count" (func $prime_count))
)
