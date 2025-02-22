(module
  (import "runtime" "init" (func $init))
  ;; TODO: 一旦大きめに確保、今後growする機能を追加
  (table (export "funcs") 10000 funcref)
  (start $init)
)
