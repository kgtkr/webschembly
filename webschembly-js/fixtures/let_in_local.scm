((lambda ()
    (define x 1)
    (define y 2)
    (write x)(newline)
    (write y)(newline)
    (let
        ((x 3))
        (define y 4)
        (write x)(newline)
        (write y)(newline)
    )
    (write x)(newline)
    (write y)(newline)
))
