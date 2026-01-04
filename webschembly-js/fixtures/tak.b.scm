(define (tak x y z)
  (define (tak-rec x y z)
    (if (not (< y x))
      z
      (tak-rec (tak-rec (- x 1) y z)
        (tak-rec (- y 1) z x)
        (tak-rec (- z 1) x y))))
  (tak-rec x y z))

(define (run)
  (tak 18 12 6))
(write (run))
(newline)
