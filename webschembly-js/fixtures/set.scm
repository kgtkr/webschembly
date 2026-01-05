;; 1. トップレベルでの代入テスト
(define x 2)
(write "initial x (2):")
(newline)
(write x)
(newline)

(write "set! result (unspecified):")
(newline)
(write (set! x 28))
(newline)

(write "updated x (28):")
(newline)
(write x)
(newline)

;; 2. ローカルな束縛（lambda/let）内での代入テスト
(write "local binding test:")
(newline)
(let ((y 10))
  (set! y (+ y 5))
  (write y))
(newline)

;; 3. 複数の代入と計算の組み合わせ
(write "complex assignment:")
(newline)
(define a 5)
(define b 10)
(set! a (+ a b))
(set! b (- a b))
(set! a (- a b))
(write (list a b))
(newline)
