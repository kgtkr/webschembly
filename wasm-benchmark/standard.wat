(module
  (func $prime_count (param $n i32) (result i32)
    (local $i i32)
    (local $j i32)
    (local $is_prime i32)
    (local $count i32)

    (local.set $count (i32.const 0))
    (local.set $i (i32.const 2))

    block $exit1
      loop $loop1
        (i32.eqz (i32.le_s (local.get $i) (local.get $n)))
        br_if $exit1

        (local.set $is_prime (i32.const 1))
        (local.set $j (i32.const 2))
        block $exit2
          loop $loop2
            (i32.eqz (i32.lt_s (local.get $j) (local.get $i)))
            br_if $exit2

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
            br $loop2
          end
        end

        (local.get $is_prime)
        if
          (local.set $count (i32.add (local.get $count) (i32.const 1)))
        end

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        br $loop1
      end
    end

    local.get $count
  )

  (export "prime_count" (func $prime_count))
)
