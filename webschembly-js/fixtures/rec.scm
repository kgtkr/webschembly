(define (sum n)
  (define (sum-rec n m)
    (if (= n 0)
        m
        (sum-rec (- n 1) (+ m n))))
  (sum-rec n 0)
)

(write (sum 100))(newline)
