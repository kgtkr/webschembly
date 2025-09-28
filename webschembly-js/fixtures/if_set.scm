(define (main)
  (define x 0)
  (if #t
      (set! x 1)
      (set! x 2))
  (write x)(newline)
  (if #f
      (set! x 3)
      (set! x 4))
  (write x)(newline)
)

(main)
