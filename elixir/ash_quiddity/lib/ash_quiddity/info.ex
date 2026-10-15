defmodule AshQuiddity.Info do
  @moduledoc """
  Introspection of the `quiddity` section, and the fields each identity of
  a resource covers.
  """

  alias AshQuiddity.Dsl.{Field, Identity}
  alias Spark.Dsl.Extension

  @doc "The resource's domain label, if it has one."
  @spec domain(Ash.Resource.t()) :: String.t() | nil
  def domain(resource), do: Extension.get_opt(resource, [:quiddity], :domain, nil)

  @doc "The section's per-field settings."
  @spec fields(Ash.Resource.t()) :: [Field.t()]
  def fields(resource), do: resource |> entities() |> Enum.filter(&match?(%Field{}, &1))

  @doc "The resource's additional identities."
  @spec identities(Ash.Resource.t()) :: [Identity.t()]
  def identities(resource), do: resource |> entities() |> Enum.filter(&match?(%Identity{}, &1))

  defp entities(resource), do: Extension.get_entities(resource, [:quiddity])

  @typedoc "A field an identity covers."
  @type entry :: %{
          name: atom,
          key: String.t(),
          field: struct,
          type: term,
          constraints: keyword,
          settings: map
        }

  @doc """
  The fields the identity `name` of `resource` covers, in no particular
  order, each with its key and settings. Raises `Quiddity.Error` for an
  identity the resource does not have.
  """
  @spec entries!(Ash.Resource.t(), atom) :: [entry]
  def entries!(resource, name) do
    # Keyed by the module's checksum too, so that recompiling the resource
    # (in development) does not leave a stale entry.
    key = {__MODULE__, resource, resource.module_info(:md5), name}

    case :persistent_term.get(key, :none) do
      :none ->
        entries = resolve!(resource, name)
        :persistent_term.put(key, entries)
        entries

      entries ->
        entries
    end
  end

  defp resolve!(resource, name) do
    identity =
      case name do
        :primary ->
          nil

        name ->
          Enum.find(identities(resource), &(&1.name == name)) ||
            raise Quiddity.Error.at("$", "#{inspect(resource)} has no identity #{inspect(name)}")
      end

    section = fn opt -> Extension.get_opt(resource, [:quiddity], opt, nil) end
    follow = (identity && identity.follow) || section.(:follow) || :core

    selection =
      cond do
        identity && (identity.only || identity.except) -> {identity.only, identity.except}
        true -> {section.(:only), section.(:except)}
      end

    settings = merge_settings(fields(resource), (identity && identity.fields) || [])

    defaults = default_fields(resource, follow)
    included = for {n, s} <- settings, s[:include], do: n

    names =
      case selection do
        {only, _} when is_list(only) -> only
        {_, except} when is_list(except) -> Enum.uniq(defaults ++ included) -- except
        _ -> Enum.uniq(defaults ++ included)
      end

    omit_nil = follow == :ash_json_api and not json_api(resource, :include_nil_values?, true)

    entries =
      for n <- names, s = Map.get(settings, n, %{}), !s[:skip] do
        field = field!(resource, n)

        s =
          if omit_nil and not Map.has_key?(s, :omit_nil), do: Map.put(s, :omit_nil, true), else: s

        %{
          name: n,
          key: s[:rename] || key(resource, n, follow),
          field: field,
          type: Map.get(field, :type),
          constraints: Map.get(field, :constraints) || [],
          settings: s
        }
      end

    keys = entries |> Enum.reject(& &1.settings[:flatten]) |> Enum.map(& &1.key)

    case keys -- Enum.uniq(keys) do
      [] ->
        entries

      [dup | _] ->
        raise Quiddity.Error.at(
                "$",
                "#{inspect(resource)}: two fields have the key #{inspect(dup)}"
              )
    end
  end

  # Each field's settings: the section's, with the identity's merged over
  # them, option by option.
  defp merge_settings(section, identity) do
    to_map = fn f ->
      f
      |> Map.from_struct()
      |> Map.drop([:name, :__spark_metadata__])
      |> Map.reject(fn {_, v} -> v == nil end)
    end

    base = Map.new(section, &{&1.name, to_map.(&1)})

    Enum.reduce(identity, base, fn f, acc ->
      Map.update(acc, f.name, to_map.(f), &Map.merge(&1, to_map.(f)))
    end)
  end

  @doc false
  def default_fields(resource, :core) do
    for a <- Ash.Resource.Info.public_attributes(resource), !a.sensitive?, do: a.name
  end

  def default_fields(resource, :ash_json_api) do
    ensure_json_api!(resource)

    fields =
      json_api(resource, :default_fields, nil) ||
        Enum.map(Ash.Resource.Info.public_attributes(resource), & &1.name)

    shown = Enum.filter(fields, &AshJsonApi.Resource.Info.show_field?(resource, &1))
    Enum.uniq(Ash.Resource.Info.primary_key(resource) ++ shown)
  end

  defp key(resource, name, :ash_json_api),
    do: AshJsonApi.Resource.Info.field_to_json_key(resource, name)

  defp key(_resource, name, :core), do: Atom.to_string(name)

  defp json_api(resource, :default_fields, _default),
    do: AshJsonApi.Resource.Info.default_fields(resource)

  defp json_api(resource, :include_nil_values?, default) do
    ensure_json_api!(resource)

    case AshJsonApi.Resource.Info.include_nil_values?(resource) do
      nil -> default
      value -> value
    end
  end

  defp ensure_json_api!(resource) do
    unless Code.ensure_loaded?(AshJsonApi.Resource.Info) and
             AshJsonApi.Resource in Spark.extensions(resource) do
      raise Quiddity.Error.at(
              "$",
              "#{inspect(resource)} follows :ash_json_api, but does not use AshJsonApi.Resource"
            )
    end
  end

  @doc false
  def field!(resource, name) do
    Ash.Resource.Info.attribute(resource, name) ||
      Ash.Resource.Info.calculation(resource, name) ||
      Ash.Resource.Info.aggregate(resource, name) ||
      Ash.Resource.Info.relationship(resource, name) ||
      raise Quiddity.Error.at("$", "#{inspect(resource)} has no field #{inspect(name)}")
  end
end
