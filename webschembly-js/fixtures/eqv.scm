(write "bool:")(newline)
;; #t同士は #t
(write (eqv? #t #t))(newline)
;; #f同士は #t
(write (eqv? #f #f))(newline)
;; #tと #fは #f
(write (eqv? #t #f))(newline)

(write "symbol:")(newline)
;; 同じ名前のシンボルは #t
(write (eqv? 'a 'a))(newline)
;; 異なる名前のシンボルは #f
(write (eqv? 'a 'b))(newline)
;; 異なる型は #f
(write (eqv? 'a '()))(newline)

(write "number:")(newline)
;; 同じ値、かつ両方とも正確なら #t
(write (eqv? 2 2))(newline)
;; 同じ値、かつ両方とも不正確なら #t
(write (eqv? 1.1 1.1))(newline)
;; 値は同じだが、正確性が異なる場合は #f
(write (eqv? 0 0.0))(newline)

(write "char:")(newline)
;; 同じ文字なら #t
(write (eqv? #\a #\a))(newline)
;; 異なる文字なら #f
(write (eqv? #\a #\b))(newline)

(write "list:")(newline)
;; 空リスト同士は #t
(write (eqv? '() '()))(newline)
;; consは新しい場所を確保するため、中身が同じでも異なるペアは #f
(write (eqv? (cons 1 2) (cons 1 2)))(newline)
;; 変数に束縛して同じ場所を指していれば #t
(let ((p (cons 1 2)))
  (write (eqv? p p)))(newline)
(write (eqv? '(a) '()))(newline)

(write "string:")(newline)
;; 変数に束縛して同じ場所を指していれば #t
(let ((s "a"))
  (write (eqv? s s)))(newline)
;; 異なる文字列は #f
(write (eqv? "a" "b"))(newline)

(write "procedure:")(newline)
;; 同じ定義場所を持つ手続き変数は #t
(write (eqv? car car))(newline)
;; lambda評価ごとに新しい手続きが生成されるため、中身が同じでも別々に評価されれば #f
(write (eqv? (lambda (x) x) (lambda (x) x)))(newline)
;; 一度生成された手続きを比較すれば #t
(let ((p (lambda (x) x)))
  (write (eqv? p p)))(newline)
