defmodule AshQuiddity.Encode do
  @moduledoc false
  # A record as a data-model value (Quiddity.Value) under one of its
  # resource's identities.

  alias Quiddity.{Quantize, Value}

  def record!(%{__struct__: resource} = record, identity) do
    entries =
      resource
      |> AshQuiddity.Info.entries!(identity)
      |> Enum.flat_map(&field!(record, &1))

    keys = Enum.map(entries, &elem(&1, 0))

    case keys -- Enum.uniq(keys) do
      [] ->
        {:map, entries}

      [dup | _] ->
        raise Quiddity.Error.at(
                "$",
                "the key #{inspect(dup)} is written twice while digesting #{inspect(resource)}: " <>
                  "a flattened field has a member with the same name as another field"
              )
    end
  end

  defp field!(record, %{name: name, key: key, settings: s} = entry) do
    path = "$." <> key

    raw =
      case Map.get(record, name) do
        %Ash.NotLoaded{} ->
          raise Quiddity.Error.at(
                  path,
                  "#{inspect(name)} is not loaded; a digest covers it, so load or select it first"
                )

        %Ash.ForbiddenField{} ->
          raise Quiddity.Error.at(
                  path,
                  "#{inspect(name)} is forbidden to this actor; a digest covering it cannot be computed"
                )

        raw ->
          raw
      end

    try do
      value = raw |> value(entry) |> as_set(s) |> quantize(s)

      cond do
        s[:flatten] -> flatten(value, path)
        s[:omit_nil] && value == nil -> []
        s[:omit_empty] && empty?(value) -> []
        true -> [{key, value}]
      end
    rescue
      e in Quiddity.Error -> reraise %{e | path: nest(path, e.path)}, __STACKTRACE__
    end
  end

  defp value(nil, _entry), do: nil

  defp value(raw, %{field: %Ash.Resource.Relationships.HasMany{}, settings: s}),
    do: records(raw, s)

  defp value(raw, %{field: %Ash.Resource.Relationships.ManyToMany{}, settings: s}),
    do: records(raw, s)

  defp value(raw, %{field: %{__struct__: rel}, settings: s})
       when rel in [Ash.Resource.Relationships.HasOne, Ash.Resource.Relationships.BelongsTo],
       do: record!(raw, s[:identity] || :primary)

  defp value(raw, %{type: type, constraints: constraints, settings: s}),
    do: typed(raw, type, constraints, s[:identity] || :primary)

  defp records(list, s), do: {:list, Enum.map(list, &record!(&1, s[:identity] || :primary))}

  @doc false
  # A value of an Ash type: a record of an embedded resource under its own
  # identity, an array item by item, anything else as its JSON-safe form
  # (Ash.Type.dump_to_embedded/3) read as Quiddity reads Elixir values.
  def typed(nil, _type, _constraints, _identity), do: nil

  # A duration dumps to ISO 8601 text; it is digested as a duration (spec 2.6).
  def typed(%Duration{} = raw, _type, _constraints, _identity), do: Value.from(raw)

  def typed(%{__struct__: module} = raw, type, constraints, identity) do
    if Ash.Resource.Info.resource?(module),
      do: record!(raw, identity),
      else: dumped(raw, type, constraints)
  end

  def typed(list, {:array, type}, constraints, identity) when is_list(list) do
    items = Keyword.get(constraints, :items, [])
    {:list, Enum.map(list, &typed(&1, type, items, identity))}
  end

  def typed(raw, type, constraints, _identity), do: dumped(raw, type, constraints)

  defp dumped(raw, type, constraints) do
    case safe_dump(type, raw, constraints) do
      {:ok, dumped} -> Value.from(dumped)
      _ -> Value.from(raw)
    end
  end

  defp safe_dump(nil, _raw, _constraints), do: :error

  defp safe_dump(type, raw, constraints) do
    Ash.Type.dump_to_embedded(type, raw, constraints)
  rescue
    _ -> :error
  end

  defp as_set({:list, items}, %{encode_as: :set}), do: {:set, items}

  defp as_set(other, %{encode_as: :set}),
    do: raise(Quiddity.Error.at("$", "encode_as: :set needs a list, not #{inspect(other)}"))

  defp as_set(value, _s), do: value

  defp quantize(value, %{quantize: step}), do: Quantize.quantize(value, step)
  defp quantize(value, %{decimals: d}), do: Quantize.decimals(value, d)
  defp quantize(value, _s), do: value

  defp flatten(nil, _path), do: []
  defp flatten({:map, entries}, _path), do: entries

  defp flatten(_other, path),
    do: raise(Quiddity.Error.at(path, "a flattened field must be a map or a record"))

  defp empty?(""), do: true
  defp empty?({tag, []}) when tag in [:list, :set, :map], do: true
  defp empty?(_), do: false

  defp nest(prefix, "$" <> rest), do: prefix <> rest
  defp nest(prefix, _), do: prefix
end
