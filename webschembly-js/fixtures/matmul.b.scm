(define (matrix-multiply a b size)
  (let ((c (make-f64vector (* size size))))
    (define (loop-k i j k sum)
      (if (< k size)
        (loop-k i j (+ k 1)
          (+ sum
            (* (uvector-ref a (+ (* i size) k))
              (uvector-ref b (+ (* k size) j)))))
        (uvector-set! c (+ (* i size) j) sum)))

    (define (loop-j i j)
      (if (< j size)
        (begin
          (loop-k i j 0 0.0)
          (loop-j i (+ j 1)))
        #t))

    (define (loop-i i)
      (if (< i size)
        (begin
          (loop-j i 0)
          (loop-i (+ i 1)))
        #t))

    (loop-i 0)
    c))

(define size 2)
(define mat-a #f64(1.0 2.0 3.0 4.0))
(define mat-b #f64(5.0 6.0 7.0 8.0))
(write (matrix-multiply mat-a mat-b size))
(newline)

(define (f64vector-iota size start step)
  (let ((vec (make-f64vector size)))
    (define (loop i value)
      (if (< i size)
        (begin
          (uvector-set! vec i value)
          (loop (+ i 1) (+ value step)))
        vec))
    (loop 0 start)))

(set! size 64)
(set! mat-a (f64vector-iota (* size size) 1000.0 0.2))
(set! mat-b (f64vector-iota (* size size) 2000.0 0.1))

(define (run)
  (matrix-multiply mat-a mat-b size))
(write (run))
(newline)
