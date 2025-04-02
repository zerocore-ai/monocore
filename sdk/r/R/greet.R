#' Greet a person
#'
#' Returns a greeting message for the given name.
#'
#' @param name A string, the name to greet.
#' @return A string containing the greeting message.
#' @export
#'
#' @examples
#' greet("World")
greet <- function(name) {
  message <- paste0("Hello, ", name, "! Welcome to Microsandbox!")
  cat(message, "\n")
  return(message)
}
