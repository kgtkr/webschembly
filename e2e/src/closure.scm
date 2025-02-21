(begin
    (define f (lambda (x) (lambda (y) x)))
    (define g (f 42))
    (define q (f 45))
    (display (g 100))
    (display (q 105))
)
