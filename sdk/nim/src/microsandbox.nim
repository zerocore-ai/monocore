## Microsandbox Nim SDK
## A minimal SDK for the Microsandbox project.

proc greet*(name: string): string =
  ## Returns a greeting message for the given name.
  ##
  ## Example:
  ##   let message = greet("World")
  ##   echo message
  let message = "Hello, " & name & "! Welcome to Microsandbox!"
  echo message
  return message

when isMainModule:
  # Simple test
  let result = greet("Test")
  assert "Hello, Test!" in result
  echo "Test passed!"
