(if #t
  (if #t
    (begin (write "1") (newline))
    (begin (write "2") (newline)))
  (if #t
    (begin (write "3") (newline))
    (begin (write "4") (newline))))
(if #f
  (if #t
    (begin (write "5") (newline))
    (begin (write "6") (newline)))
  (if #f
    (begin (write "7") (newline))
    (begin (write "8") (newline))))
(if #t
  (if #f
    (begin (write "9") (newline))
    (begin (write "10") (newline)))
  (if #t
    (begin (write "11") (newline))
    (begin (write "12") (newline))))
(if #f
  (if #f
    (begin (write "13") (newline))
    (begin (write "14") (newline)))
  (if #f
    (begin (write "15") (newline))
    (begin (write "16") (newline))))
