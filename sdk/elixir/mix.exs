defmodule Microsandbox.MixProject do
  use Mix.Project

  def project do
    [
      app: :microsandbox,
      version: "0.1.0",
      elixir: "~> 1.12",
      start_permanent: Mix.env() == :prod,
      description: "A minimal Elixir SDK for the Microsandbox project",
      package: package(),
      deps: deps(),
      name: "Microsandbox",
      source_url: "https://github.com/microsandbox/microsandbox"
    ]
  end

  def application do
    [
      extra_applications: [:logger]
    ]
  end

  defp deps do
    [
      {:ex_doc, "~> 0.24", only: :dev, runtime: false}
    ]
  end

  defp package do
    [
      maintainers: ["Microsandbox Team"],
      licenses: ["Apache-2.0"],
      links: %{
        "GitHub" => "https://github.com/microsandbox/microsandbox",
        "Website" => "https://microsandbox.dev"
      }
    ]
  end
end
