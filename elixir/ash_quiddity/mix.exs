defmodule AshQuiddity.MixProject do
  use Mix.Project

  @version "0.1.0"
  @source_url "https://github.com/univalent-cc/quiddity"

  def project do
    [
      app: :ash_quiddity,
      version: @version,
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      # Tests and the example scripts define resources, whose Quiddity.Encoder
      # implementations consolidated protocols would ignore.
      consolidate_protocols: Mix.env() == :prod,
      elixirc_paths: elixirc_paths(Mix.env()),
      deps: deps(),
      description:
        "Quiddity digests for Ash resources: the resource's definition as its identity.",
      package: package(),
      docs: docs(),
      source_url: @source_url
    ]
  end

  def application, do: [extra_applications: [:logger]]

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  defp deps do
    [
      {:quiddity, path: "../quiddity"},
      {:ash, "~> 3.0"},
      {:ash_json_api, "~> 1.0", optional: true},
      {:ex_doc, "~> 0.34", only: :dev, runtime: false}
    ]
  end

  defp package do
    [
      licenses: ["MIT"],
      links: %{"GitHub" => @source_url},
      files: ~w(lib mix.exs README.md LICENSE)
    ]
  end

  defp docs do
    [main: "readme", source_ref: "v#{@version}", extras: ["README.md"]]
  end
end
