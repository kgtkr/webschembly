(define (matrix-multiply a b size)
  (let ((c (make-f64vector (* size size))))
    (define (loop-k i j k sum a b size c)
      (if (< k size)
        (loop-k i j (+ k 1)
          (+ sum
            (* (uvector-ref a (+ (* i size) k))
              (uvector-ref b (+ (* k size) j))))
          a
          b
          size
          c)
        (uvector-set! c (+ (* i size) j) sum)))

    (define (loop-j i j a b size c)
      (if (< j size)
        (begin
          (loop-k i j 0 0.0 a b size c)
          (loop-j i (+ j 1) a b size c))
        #t))

    (define (loop-i i a b size c)
      (if (< i size)
        (begin
          (loop-j i 0 a b size c)
          (loop-i (+ i 1) a b size c))
        #t))

    (loop-i 0 a b size c)
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

(define size 64)
(define mat-a (f64vector-iota (* size size) 1000.0 0.2))
(define mat-b (f64vector-iota (* size size) 2000.0 0.1))
(write (matrix-multiply mat-a mat-b size))
(newline)
(define (run)
  (matrix-multiply mat-a mat-b size))
(write "start")
(newline)
(run)
(write "done")
(newline)
