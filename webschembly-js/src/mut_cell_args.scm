(define (f a)
  (define (g)
    (set! a 2))
  (write a)(newline)
  (g)
  (write a)(newline)
  a
)
(write (f 1))(newline)
