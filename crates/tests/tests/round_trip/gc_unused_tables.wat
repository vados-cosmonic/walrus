;; Can remove an unused table.

(module
  (type (;0;) (func (result i32)))
  (table 1 1 anyfunc)
  (func $f (type 0) (result i32)
    i32.const 42)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      i32.const 42
    )
    (export "f" (func $f))
;)
