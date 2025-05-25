(module
  (func $bb_entry (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.set $count (i32.const 0))
    (local.set $i (i32.const 2))

    (return_call $bb_loop1_cond (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )
  
  (func $bb_loop1_cond (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (i32.le_s (local.get $i) (local.get $n))
    if
      (return_call $bb_loop1_body (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
    else
      (return (local.get $count))
    end
  )

  (func $bb_loop1_body (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.set $is_prime (i32.const 1))
    (local.set $j (i32.const 2))
    (return_call $bb_loop2_cond (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )
  
  (func $bb_loop1_body2 (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.get $is_prime)
    if
      (return_call $bb_if1_then (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
    else
      (return_call $bb_loop1_body3 (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
    end
  )

  (func $bb_if1_then (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.set $count (i32.add (local.get $count) (i32.const 1)))
    (return_call $bb_loop1_body3 (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )

  (func $bb_loop1_body3 (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.set $i (i32.add (local.get $i) (i32.const 1)))
    (return_call $bb_loop1_cond (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )

  (func $bb_loop2_cond (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (i32.lt_s (local.get $j) (local.get $i))
    if
      (return_call $bb_loop2_body (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
    else
      (return_call $bb_loop1_body2 (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
    end
  )

  (func $bb_loop2_body (param $n i32) (param $i i32) (param $j i32) (param $is_prime i32) (param $count i32) (result i32)
    (local.set $is_prime
      (i32.and
        (local.get $is_prime)
        (i32.ne
          (i32.rem_u (local.get $i) (local.get $j))
          (i32.const 0)
        )
      )
    )

    (local.set $j (i32.add (local.get $j) (i32.const 1)))
    (return_call $bb_loop2_cond (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )

  (func $prime_count (param $n i32) (result i32)
    (local $i i32)
    (local $j i32)
    (local $is_prime i32)
    (local $count i32)

    (return_call $bb_entry (local.get $n) (local.get $i) (local.get $j) (local.get $is_prime) (local.get $count))
  )

  (export "prime_count" (func $prime_count))
)
