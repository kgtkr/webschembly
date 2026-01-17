(define (create-n size)
  (define (loop n acc)
    (if (= n 0)
      acc
      (loop (- n 1) (cons n acc))))
  (loop size '()))

(define (div2 l)
  (define (loop x y z)
    (if (null? x)
      (cons y (cons z '()))
      (if (null? (cdr x))
        (cons (cons (car x) y) (cons z '()))
        (loop (cddr x)
          (cons (car x) y)
          (cons (cadr x) z)))))
  (loop l '() '()))

(define l (create-n 1000))

(define (run)
  (div2 l))

(write (run))
(newline)
