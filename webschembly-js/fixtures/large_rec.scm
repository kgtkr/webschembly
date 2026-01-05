(define (rec n)
  (if (= n 0)
    0
    (rec (- n 1))))
(write (rec 1000000))
(newline)
