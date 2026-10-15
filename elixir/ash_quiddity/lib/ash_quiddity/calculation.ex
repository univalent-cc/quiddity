defmodule AshQuiddity.Calculation do
  @moduledoc """
  A calculation giving a record's digest, as text.

      calculations do
        calculate :etag, :string, {AshQuiddity.Calculation, identity: :primary, form: :micro}
      end

  The digest is computed only when the calculation is loaded
  (`Ash.load/2`, or a query's `load`), and Ash first loads the fields the
  digest covers.

  ## Options

    * `:identity` - the identity the digest is taken under; `:primary` by
      default.
    * `:form` - `:full` (the default, 256 bits), `:mini` (128 bits) or
      `:micro` (64 bits).
  """

  use Ash.Resource.Calculation

  @impl true
  def init(opts) do
    form = Keyword.get(opts, :form, :full)

    if form in [:full, :mini, :micro],
      do: {:ok, Keyword.merge([identity: :primary, form: :full], opts)},
      else: {:error, "`form` must be :full, :mini or :micro"}
  end

  @impl true
  def describe(opts),
    do: "the digest of the record under its identity #{inspect(opts[:identity])}"

  @impl true
  def load(query, opts, _context) do
    query.resource
    |> AshQuiddity.Info.entries!(opts[:identity])
    |> Enum.map(fn
      %{name: name, field: %{destination: _}} -> {name, []}
      %{name: name} -> name
    end)
  end

  @impl true
  def calculate(records, opts, _context) do
    Enum.map(records, fn record ->
      digest = Quiddity.digest!(record, opts[:identity])

      case opts[:form] do
        :full -> to_string(digest)
        :mini -> to_string(Quiddity.Digest.mini(digest))
        :micro -> to_string(Quiddity.Digest.micro(digest))
      end
    end)
  end
end
