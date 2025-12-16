(define (f . args)
    (write "f:")(newline)
    (write args)(newline)
)

(define (g x1 x2 . args)
    (write "g:")(newline)
    (write x1)(newline)
    (write x2)(newline)
    (write args)(newline)
)

(f 1 2 3 4 5)
(g 1 2 3 4 5)

(g 1)
