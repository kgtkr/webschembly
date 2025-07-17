(begin
    (define f (lambda (x) (lambda (y) x)))
    (define g (f 42))
    (define q (f 45))
    (write (g 100))(newline)
    (write (q 105))(newline)
)
