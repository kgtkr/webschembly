(begin
    (define f (lambda (x) (lambda (y) x)))
    (define g (f 42))
    (define q (f 45))
    (dump (g 100))
    (dump (q 105))
)
