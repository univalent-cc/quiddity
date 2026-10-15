defmodule AshQuiddity.Export do
  @moduledoc false
  # A resource's identity as data (Quiddity.Identity): the identity another
  # language applies to a record's JSON form. Ash types map to kinds as
  # history/ELIXIR.md (E7) describes.

  alias Quiddity.Identity

  def identity!(resource, name) do
    fields =
      resource
      |> AshQuiddity.Info.entries!(name)
      |> Enum.map(&field!(resource, &1))

    short = resource |> Module.split() |> List.last()

    {:object, fields}
    |> Identity.new!(domain: AshQuiddity.Info.domain(resource))
    |> Identity.named(short)
  end

  defp field!(resource, %{key: key, field: field, settings: s} = entry) do
    if s[:flatten] do
      raise Quiddity.Error.at(
              "$.#{key}",
              "#{inspect(resource)}: a flattened field cannot be expressed in an identity document"
            )
    end

    kind = kind(entry)

    kind =
      case {s[:encode_as], kind} do
        {:set, {:list, item}} -> {:set, item}
        {:set, other} -> {:set, other}
        {_, kind} -> kind
      end

    nullable? = Map.get(field, :allow_nil?, true)

    {kind, opts} =
      cond do
        s[:omit_empty] -> {kind, [omit_empty: true]}
        s[:omit_nil] -> {kind, [optional: true]}
        nullable? and kind != :any -> {{:optional, kind}, []}
        true -> {kind, []}
      end

    opts =
      opts ++ Enum.reject([quantize: s[:quantize], decimals: s[:decimals]], &(elem(&1, 1) == nil))

    {key, Identity.field(kind, opts)}
  end

  defp kind(%{field: %Ash.Resource.Relationships.HasMany{destination: d}, settings: s}),
    do: {:list, nested(d, s[:identity])}

  defp kind(%{field: %Ash.Resource.Relationships.ManyToMany{destination: d}, settings: s}),
    do: {:list, nested(d, s[:identity])}

  defp kind(%{field: %{__struct__: rel, destination: d}, settings: s})
       when rel in [Ash.Resource.Relationships.HasOne, Ash.Resource.Relationships.BelongsTo],
       do: nested(d, s[:identity])

  defp kind(%{type: type, constraints: constraints, settings: s}),
    do: type_kind(type, constraints, s[:identity])

  defp nested(resource, identity), do: identity!(resource, identity || :primary)

  @doc false
  def type_kind(nil, _constraints, _identity), do: :any

  def type_kind({:array, type}, constraints, identity),
    do: {:list, type_kind(type, Keyword.get(constraints, :items, []), identity)}

  def type_kind(type, constraints, identity) do
    type = Ash.Type.get_type(type)

    cond do
      Ash.Type.NewType.new_type?(type) ->
        type_kind(Ash.Type.NewType.subtype_of(type), constraints, identity)

      Ash.Resource.Info.resource?(type) ->
        nested(type, identity)

      type == Ash.Type.Struct and Ash.Resource.Info.resource?(constraints[:instance_of]) ->
        nested(constraints[:instance_of], identity)

      type in [Ash.Type.Map, Ash.Type.Struct] and is_list(constraints[:fields]) ->
        {:object,
         Enum.map(constraints[:fields], fn {name, config} ->
           k = type_kind(config[:type], config[:constraints] || [], nil)
           {name, if(Keyword.get(config, :allow_nil?, true), do: {:optional, k}, else: k)}
         end)}

      Spark.implements_behaviour?(type, Ash.Type.Enum) ->
        :string

      true ->
        Map.get(kinds(), type, :any)
    end
  end

  # Ash's built-in types with a data-model kind, or digested as their text
  # (history/ELIXIR.md, E7).
  defp kinds do
    %{
      Ash.Type.String => :string,
      Ash.Type.CiString => :string,
      Ash.Type.Integer => :integer,
      Ash.Type.Float => :number,
      Ash.Type.Boolean => :bool,
      Ash.Type.Atom => :string,
      Ash.Type.UUID => :uuid,
      Ash.Type.UUIDv7 => :uuid,
      Ash.Type.UtcDatetime => :instant,
      Ash.Type.UtcDatetimeUsec => :instant,
      Ash.Type.DateTime => :instant,
      Ash.Type.NaiveDatetime => :string,
      Ash.Type.Date => :string,
      Ash.Type.Time => :string,
      Ash.Type.TimeUsec => :string,
      Ash.Type.Duration => :string,
      Ash.Type.DurationName => :string,
      Ash.Type.Decimal => :string,
      Ash.Type.Module => :string,
      Ash.Type.Binary => :string
    }
  end
end
