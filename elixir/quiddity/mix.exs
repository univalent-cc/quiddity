defmodule Quiddity.MixProject do
  use Mix.Project

  @version "0.1.0"
  @source_url "https://github.com/univalent-cc/quiddity"

  def project do
    [
      app: :quiddity,
      version: @version,
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      # Tests and the example scripts define structs that derive
      # Quiddity.Encoder, which consolidated protocols would ignore.
      consolidate_protocols: Mix.env() == :prod,
      elixirc_paths: elixirc_paths(Mix.env()),
      deps: deps(),
      description: "Stable, configurable digests that capture the essence of values.",
      package: package(),
      docs: docs(),
      source_url: @source_url
    ]
  end

  # Test structs are compiled, so that their typespecs can be read.
  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  def application do
    [extra_applications: [:crypto]]
  end

  defp deps do
    [
      {:jason, "~> 1.4", optional: true},
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
    [
      main: "readme",
      source_ref: "v#{@version}",
      extras: ["README.md"],
      groups_for_modules: [
        Digests: [Quiddity, Quiddity.Digest, Quiddity.Value, Quiddity.Encoder],
        Identities: [Quiddity.Identity],
        Errors: [Quiddity.Error, Quiddity.InvalidIdentity]
      ]
    ]
  end
end
