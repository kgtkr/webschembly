;; (<test> <expression> ...)
(cond (#t
        (write "Basic: First clause match")
        (newline))
      (#t
        (write "Basic: Should not be reached")
        (newline)))
(cond (#f
        (write "Skip me")
        (newline))
      (#t
        (write "Basic: Second clause match")
        (newline)))
(cond (#f
        (write "Skip me")
        (newline))
      (#f
        (write "Skip me too")
        (newline))
      (else
        (write "Else: Fallback match")
        (newline)))

;; (<test>) の場合 (<test> <test>) とほぼ同じ動作をする
(write
  (cond (10)))
(newline)

(write
  (cond (#f) (20)))
(newline)

;; ただし<test>は一度だけ評価される
(cond
  ((begin (write "Evaluated once")(newline) #t)))


;; (<test> => <expression>) の場合
(cond (100 => 
       (lambda (x) 
         (write "Arrow: Received value ") 
         (write x) 
         (newline))))

(cond (#f => (lambda (x) (write "Should not run")))
      (else 
        (write "Arrow: Skipped false test")
        (newline)))

;; <test> は一度だけ評価される
(cond
  ((begin (write "Arrow evaluated once")(newline) 200) =>
   (lambda (x)
     (write "Arrow received ")
     (write x)
     (newline))))
