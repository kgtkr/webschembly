(define (tak x y z)
  (define (tak-cps x y z k)
    (if (not (< y x))
      (k z)
      (tak-cps (- x 1) y z
        (lambda (v1)
          (tak-cps (- y 1) z x
            (lambda (v2)
              (tak-cps (- z 1) x y
                (lambda (v3)
                  (tak-cps v1 v2 v3 k)))))))))
  (tak-cps x y z (lambda (x) x)))

(define (run)
  (tak 18 12 6))
(write (run))
(newline)
