(** Microsandbox OCaml SDK *)

(** Returns a greeting message for the given name. *)
let greet name =
  let message = "Hello, " ^ name ^ "! Welcome to Microsandbox!" in
  print_endline message;
  message
