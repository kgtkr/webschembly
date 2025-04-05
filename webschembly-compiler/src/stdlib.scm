(define (newline)
  (write-char #\newline))
(define (write x)
  (define (write-vector-inner v i)
    (write (vector-ref v i))
    (if (< (+ i 1) (vector-length v))
        (begin
          (write-char #\space)
          (write-vector-inner v (+ i 1)))
        #f)
  )
  (if (pair? x)
      (begin
        (write-char #\openparen)
        (write (car x))
        (display " . ")
        (write (cdr x))
        (write-char #\closeparen))
  (if (eq? x #t)
      (display "#t")
  (if (eq? x #f)
      (display "#f")
  (if (eq? x '())
      (display "()")
  (if (symbol? x)
      (display (symbol->string x))
  (if (string? x)
      (begin
        (write-char #\")
        (display x)
        (write-char #\"))
  (if (number? x)
      (display (number->string x))
  (if (char? x)
      (begin
        (write-char #\#)
        (write-char #\\)
        (write-char x))
  (if (procedure? x)
      (display "<procedure>")
  (if (vector? x)
      (begin
        (write-char #\#)
        (write-char #\openparen)
        (if (= (vector-length x) 0)
            #f
            (write-vector-inner x 0))
        (write-char #\closeparen))
  (display "<unknown>")))))))))))
)
