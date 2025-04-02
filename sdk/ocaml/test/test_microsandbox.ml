let () =
  let result = Microsandbox.greet "Test" in
  assert (String.contains result "Hello, Test!");
  print_endline "Test passed!"
