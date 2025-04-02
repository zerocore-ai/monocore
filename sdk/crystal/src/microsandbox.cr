require "./microsandbox/version"

# Main module for the Microsandbox library
module Microsandbox
  VERSION = "0.0.1"

  # Returns a greeting message with the given name
  #
  # ## Example
  #
  # ```
  # Microsandbox.greet("World") # => "Hello, World!"
  # ```
  def self.greet(name : String) : String
    "Hello, #{name}!"
  end
end
