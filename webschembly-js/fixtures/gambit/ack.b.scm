;;; ACK -- One of the Kernighan and Van Wyk benchmarks.

(define (ack m n)
  (cond ((= m 0) (+ n 1))
    ((= n 0) (ack (- m 1) 1))
    (else (ack (- m 1) (ack m (- n 1))))))

(define arg 9)

(define (run arg)
  (ack 3 arg))

(write (run arg))
(newline)
