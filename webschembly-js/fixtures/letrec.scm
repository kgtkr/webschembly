(letrec
  ((factorial
      (lambda (n)
        (if (= n 0)
          1
          (* n (factorial (- n 1)))))))
  (write (factorial 5))
  (newline))
