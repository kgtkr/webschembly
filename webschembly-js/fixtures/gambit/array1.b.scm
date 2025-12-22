;;; ARRAY1 -- One of the Kernighan and Van Wyk benchmarks.

(define (create-x n)
  (define result (make-vector n))
  (do ((i 0 (+ i 1)))
    ((>= i n) result)
    (vector-set! result i i)))

(define (create-y x)
  (let* ((n (vector-length x))
         (result (make-vector n)))
    (do ((i (- n 1) (- i 1)))
      ((< i 0) result)
      (vector-set! result i (vector-ref x i)))))

(define (my-try n)
  (vector-length (create-y (create-x n))))

(define (go n)
  (let loop ((repeat 100)
             (result '()))
    (if (> repeat 0)
      (loop (- repeat 1) (my-try n))
      result)))

(write (go 200000))
(newline)

(define (loop n)
  (if (= n 0)
    '()
    (begin
      (go 200000)
      (loop (- n 1)))))

(define (run)
  (loop 3))

(write "start")
(newline)
(run)
(write "done")
(newline)
