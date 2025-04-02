defmodule MicrosandboxTest do
  use ExUnit.Case
  doctest Microsandbox

  test "greet/1 returns the correct message" do
    result = Microsandbox.greet("Test")
    assert result =~ "Hello, Test!"
  end
end
