(define (tak x y z)
  (define (tak-rec x y z)
    (if (not (< y x))
        z
        (tak-rec (tak-rec (- x 1) y z)
            (tak-rec (- y 1) z x)
            (tak-rec (- z 1) x y))))
  (tak-rec x y z)
)
(write (tak 18 12 6))(newline)

(define (loop n)
  (if (= n 0)
      '()
      (begin
        (tak 18 12 6)
        (loop (- n 1)))))
(define (main)
  (loop 30))

(write "start")(newline)
(main)
(write "done")(newline)
