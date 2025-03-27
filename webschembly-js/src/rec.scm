(define (sum n m)
  (if (= n 0)
      m
      (sum (- n 1) (+ m n))))
(write (sum 100 0))(newline)
