// prog.monkey - differential test (inline comments only; see note in report). Deterministic output: hashes are read by key, never printed whole.
let fib = fn(n) { if (n < 2) { return n; } fib(n - 1) + fib(n - 2); };
puts(fib(15));                                  // 610
let map = fn(arr, f) { let iter = fn(a, acc) { if (len(a) == 0) { acc } else { iter(rest(a), push(acc, f(first(a)))); } }; iter(arr, []); };
puts(map([1, 2, 3, 4], fn(x) { x * x }));       // [1, 4, 9, 16]
let adder = fn(x) { fn(y) { x + y } };
puts(adder(10)(5));                             // 15
let person = {"name": "Monkey", "age": 1 + 2};
puts(person["name"]);                           // Monkey
puts(person["age"]);                            // 3
puts("a" + "b" + "c");                          // abc
puts(len("hello world"));                       // 11
puts(1.5 + 2.25);                               // 3.75
puts(10 / 3);                                   // 3 (integer truncation)
puts(-7 / 2);                                   // -3
puts(2 * (3 + 4));                              // 14
puts(true == false);                            // false
puts(1 < 2);                                    // true
puts(!5);                                        // false
puts(!!0);                                       // true
puts([10, 20, 30][1]);                          // 20
puts([1, 2, 3][5]);                             // nil (out of bounds)
let r = adder(100)(23);
[r, r + 1, "done"]                              // final value: [123, 124, done]
