defmodule Microsandbox do
  @moduledoc """
  A minimal SDK for the Microsandbox project.
  """

  @doc """
  Returns a greeting message for the given name.

  ## Examples

      iex> Microsandbox.greet("World")
      "Hello, World! Welcome to Microsandbox!"

  """
  @spec greet(String.t()) :: String.t()
  def greet(name) do
    message = "Hello, #{name}! Welcome to Microsandbox!"
    IO.puts(message)
    message
  end
end
