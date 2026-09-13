;; 脱糖処理（do, or, cond 等）で内部的に導入される一時変数（"loop", "temp"）と
;; 同名の識別子がユーザーコード側で定義・使用されている場合でも、
;; 色付き識別子によって変数キャプチャが発生せず、
;; 外側のユーザー変数を正しく参照できること（衛生性: hygiene）を検証する。

;; 1. do の脱糖: 脱糖が導入するループ関数名 "loop" が、外側のユーザー変数 `loop` をキャプチャ・シャドウイングしないか
(let ((loop 5))
  (write (do ((i 0 (+ i 1)))
          ((= i loop) i)))
  (newline)
  (write loop)
  (newline))

;; 2. or の脱糖: 脱糖が導入する一時変数 "temp" が、外側のユーザー変数 `temp` をキャプチャ・シャドウイングしないか
(let ((temp 42))
  (write (or #f temp))
  (newline)
  (write temp)
  (newline))

;; 3. cond の脱糖: 送信節 (=>) で脱糖が導入する一時変数 "temp" が、外側のユーザー変数 `temp` をキャプチャ・シャドウイングしないか
(let ((temp 99))
  (write (cond (#f #f)
          (temp => (lambda (x) (+ x 1)))
          (else #f)))
  (newline)
  (write temp)
  (newline))
