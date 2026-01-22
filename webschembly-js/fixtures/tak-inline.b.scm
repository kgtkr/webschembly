(define (tak x y z)
  (define (tak-rec x y z)
    (if (>= y x)
      z
      (tak-rec (tak-rec (- x 1) y z)
        (tak-rec (- y 1) z x)
        (tak-rec (- z 1) x y))))
  (tak-rec x y z))

(define arg 6)

(define (run arg)
  (tak 18 12 arg))
(write (run arg))
(newline)
