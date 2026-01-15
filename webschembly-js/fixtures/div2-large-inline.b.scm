(define (create-n size)
  (define (loop n acc)
    (if (= n 0)
      acc
      (loop (- n 1) (cons n acc))))
  (loop size '()))

(define (div2 l)
  (define (loop x y z)
    (if (eq? x '())
      (cons y (cons z '()))
      (if (eq? (cdr x) '())
        (cons (cons (car x) y) (cons z '()))
        (loop (cdr (cdr x))
          (cons (car x) y)
          (cons (car (cdr x)) z)))))
  (loop l '() '()))

(define l (create-n 1000))

(define (run)
  (div2 l))

(write (run))
(newline)
