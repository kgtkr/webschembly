export const examples: Record<string, string> = {
    "sum": `(define (sum n)
  (define (sum-rec n m)
    (if (= n 0)
      m
      (sum-rec (- n 1) (+ m n))))
  (sum-rec n 0))

(write (sum 100))
(newline)
`,
    "tak": `(define (tak x y z)
  (define (tak-rec x y z)
    (if (not (< y x))
      z
      (tak-rec (tak-rec (- x 1) y z)
        (tak-rec (- y 1) z x)
        (tak-rec (- z 1) x y))))
  (tak-rec x y z))

(write (tak 18 12 6))
(newline)
`,
    "matmul": `(define (matrix-multiply a b size)
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
`
};
