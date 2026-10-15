defmodule Quiddity.Derive do
  @moduledoc false
  # The derived implementation of Quiddity.Encoder: its options are checked
  # and resolved per identity when the struct is compiled, and encode/3 is
  # the shared code every derived implementation calls.

  alias Quiddity.{Quantize, Value}

  @type_options [:only, :except, :domain, :rename_all, :fields, :identities]
  @identity_options [:only, :except, :fields]
  @field_options [
    :skip,
    :rename,
    :omit_nil,
    :omit_empty,
    :quantize,
    :decimals,
    :encode_as,
    :identity,
    :flatten,
    :kind
  ]
  @rules [
    :camel_case,
    :pascal_case,
    :snake_case,
    :kebab_case,
    :screaming_snake_case,
    :screaming_kebab_case,
    :lowercase,
    :uppercase
  ]

  @doc false
  def config!(module, fields, opts) do
    opts = if opts == [], do: [], else: opts
    check_keys!(module, opts, @type_options, "option")
    domain = Keyword.get(opts, :domain)

    if domain != nil and (not is_binary(domain) or domain == "") do
      fail!(module, "`domain` must be a non-empty string")
    end

    rule = Keyword.get(opts, :rename_all)

    if rule != nil and rule not in @rules do
      fail!(module, "`rename_all` must be one of #{inspect(@rules)}")
    end

    base_fields = Keyword.get(opts, :fields, [])
    check_fields!(module, fields, base_fields)
    identities = Keyword.get(opts, :identities, [])

    if Keyword.has_key?(identities, :primary) do
      fail!(module, "the primary identity is configured at the top level, not under `identities`")
    end

    primary = resolve!(module, fields, rule, Keyword.take(opts, [:only, :except]), base_fields)

    resolved =
      Enum.reduce(identities, %{primary: primary}, fn {name, id_opts}, acc ->
        check_keys!(module, id_opts, @identity_options, "identity option")
        own_fields = Keyword.get(id_opts, :fields, [])
        check_fields!(module, fields, own_fields)
        selection = Keyword.take(id_opts, [:only, :except])
        selection = if selection == [], do: Keyword.take(opts, [:only, :except]), else: selection
        merged = merge_fields(base_fields, own_fields)
        Map.put(acc, name, resolve!(module, fields, rule, selection, merged))
      end)

    %{module: module, domain: domain, identities: resolved}
  end

  defp merge_fields(base, own) do
    Enum.reduce(own, base, fn {name, field_opts}, acc ->
      Keyword.update(acc, name, field_opts, &Keyword.merge(&1, field_opts))
    end)
  end

  defp resolve!(module, fields, rule, selection, field_opts) do
    selected =
      case selection do
        [only: only] -> Enum.filter(fields, &(&1 in only))
        [except: except] -> Enum.reject(fields, &(&1 in except))
        [] -> fields
        _ -> fail!(module, "`only` and `except` cannot be combined")
      end

    entries =
      for field <- selected,
          opts = Keyword.get(field_opts, field, []),
          not Keyword.get(opts, :skip, false) do
        check_field!(module, field, opts)
        key = Keyword.get(opts, :rename) || rename(Atom.to_string(field), rule)
        %{field: field, key: key, opts: opts}
      end

    keys = entries |> Enum.reject(& &1.opts[:flatten]) |> Enum.map(& &1.key)

    case keys -- Enum.uniq(keys) do
      [] -> entries
      [dup | _] -> fail!(module, "two fields have the key #{inspect(dup)}")
    end
  end

  defp check_field!(module, field, opts) do
    if opts[:quantize] != nil and opts[:decimals] != nil,
      do: fail!(module, "field #{inspect(field)} has quantize or decimals, not both")

    if q = opts[:quantize],
      do:
        unless(is_integer(q) and q >= 1,
          do: fail!(module, "`quantize` must be a positive integer")
        )

    if d = opts[:decimals],
      do:
        unless(is_integer(d) and d in -300..300,
          do: fail!(module, "`decimals` must be an integer from -300 to 300")
        )

    if (e = opts[:encode_as]) && e != :set,
      do: fail!(module, "a field's `encode_as` must be `:set`")

    if opts[:flatten] &&
         Enum.any?([:rename, :omit_nil, :omit_empty, :quantize, :decimals, :encode_as], &opts[&1]),
       do: fail!(module, "`flatten` cannot be combined with options for a field's key or value")

    if (r = opts[:rename]) && not (is_binary(r) and r != ""),
      do: fail!(module, "`rename` must be a non-empty string")

    :ok
  end

  defp check_fields!(module, fields, field_opts) do
    unless Keyword.keyword?(field_opts),
      do: fail!(module, "`fields` must be a keyword list of field names and options")

    for {name, opts} <- field_opts do
      unless name in fields, do: fail!(module, "#{inspect(name)} is not a field")
      check_keys!(module, opts, @field_options, "option for field #{inspect(name)}")
    end
  end

  defp check_keys!(module, opts, allowed, what) do
    unless Keyword.keyword?(opts), do: fail!(module, "expected a keyword list of #{what}s")

    case Keyword.keys(opts) -- allowed do
      [] ->
        :ok

      [k | _] ->
        fail!(module, "unknown #{what} #{inspect(k)}; expected one of #{inspect(allowed)}")
    end
  end

  defp fail!(module, message),
    do:
      raise(ArgumentError, "invalid @derive Quiddity.Encoder for #{inspect(module)}: #{message}")

  @doc false
  def encode(struct, identity, %{identities: identities, module: module}) do
    entries =
      case identities do
        %{^identity => entries} ->
          entries

        _ ->
          raise Quiddity.Error.at("$", "#{inspect(module)} has no identity #{inspect(identity)}")
      end

    {:map, Enum.flat_map(entries, &field(struct, &1)) |> unique!(module)}
  end

  @doc false
  def domain(%{domain: domain}), do: domain

  @doc false
  def export!(module, name) do
    config = config_of!(module)

    entries =
      case config.identities do
        %{^name => entries} -> entries
        _ -> raise Quiddity.Error.at("$", "#{inspect(module)} has no identity #{inspect(name)}")
      end

    short = module |> Module.split() |> List.last()
    exporting = Process.get(:quiddity_exporting, [])
    Process.put(:quiddity_exporting, [module | exporting])

    try do
      kinds = Quiddity.Typespec.field_kinds(module)
      fields = Enum.map(entries, &export_field!(module, &1, kinds))

      {:object, fields}
      |> Quiddity.Identity.new!(domain: config.domain)
      |> Quiddity.Identity.named(short)
    after
      Process.put(:quiddity_exporting, exporting)
    end
  end

  defp config_of!(module) do
    if Code.ensure_loaded?(module) and function_exported?(module, :__quiddity__, 1),
      do: module.__quiddity__(:config),
      else: raise(Quiddity.Error.at("$", "#{inspect(module)} does not derive Quiddity.Encoder"))
  end

  defp export_field!(module, %{field: field, key: key, opts: opts}, typespec_kinds) do
    if opts[:flatten],
      do:
        raise(
          Quiddity.Error.at(
            "$.#{key}",
            "#{inspect(module)}: a flattened field cannot be expressed in an identity document"
          )
        )

    # A declared `kind:`, else the struct's typespec, else :any.
    kind = export_kind(opts[:kind] || Map.get(typespec_kinds, field, :any), opts)

    kind =
      if opts[:encode_as] == :set do
        case kind do
          {:list, item} -> {:set, item}
          :any -> {:set, :any}
          other -> other
        end
      else
        kind
      end

    field_opts =
      [
        omit_empty: opts[:omit_empty],
        optional: opts[:omit_nil] && !opts[:omit_empty],
        quantize: opts[:quantize],
        decimals: opts[:decimals]
      ]
      |> Enum.reject(fn {_, v} -> v in [nil, false] end)

    {key, Quiddity.Identity.field(kind, field_opts)}
  end

  # A field's declared kind; a module deriving Quiddity.Encoder stands for
  # its own exported identity.
  defp export_kind(:recursive, _opts), do: :recursive

  defp export_kind(kind, opts) when is_atom(kind) do
    if Code.ensure_loaded?(kind) and function_exported?(kind, :__quiddity__, 1),
      do: Quiddity.identity!(kind, Keyword.get(opts, :identity, :primary)),
      else: kind
  end

  defp export_kind(kind, _opts), do: kind

  defp field(struct, %{field: name, key: key, opts: opts}) do
    path = "$." <> key
    raw = Map.fetch!(struct, name)

    try do
      value = value(raw, opts) |> quantize(opts)

      cond do
        opts[:flatten] -> flatten(value, path)
        opts[:omit_nil] && value == nil -> []
        opts[:omit_empty] && empty?(value) -> []
        true -> [{key, value}]
      end
    rescue
      e in Quiddity.Error -> reraise %{e | path: nest(path, e.path)}, __STACKTRACE__
    end
  end

  defp value(raw, opts) do
    value =
      case {opts[:identity], raw} do
        {nil, raw} -> Value.from(raw)
        {id, %{__struct__: _} = struct} -> Quiddity.Encoder.encode(struct, id)
        {_id, raw} -> Value.from(raw)
      end

    case {opts[:encode_as], value} do
      {:set, {:list, items}} ->
        {:set, items}

      {:set, other} ->
        raise Quiddity.Error.at("$", "encode_as: :set needs a list, not #{inspect(other)}")

      _ ->
        value
    end
  end

  defp quantize(value, opts) do
    cond do
      step = opts[:quantize] -> Quantize.quantize(value, step)
      d = opts[:decimals] -> Quantize.decimals(value, d)
      true -> value
    end
  end

  defp flatten(nil, _path), do: []
  defp flatten({:map, entries}, _path), do: entries

  defp flatten(_other, path),
    do: raise(Quiddity.Error.at(path, "a flattened field must be a map or a struct"))

  defp unique!(entries, module) do
    keys = Enum.map(entries, &elem(&1, 0))

    case keys -- Enum.uniq(keys) do
      [] ->
        entries

      [dup | _] ->
        raise Quiddity.Error.at(
                "$",
                "the key #{inspect(dup)} is written twice while digesting #{inspect(module)}: " <>
                  "a flattened field has a member with the same name as another field"
              )
    end
  end

  defp empty?(""), do: true
  defp empty?({:list, []}), do: true
  defp empty?({:set, []}), do: true
  defp empty?({:map, []}), do: true
  defp empty?(_), do: false

  defp nest(prefix, "$" <> rest), do: prefix <> rest
  defp nest(prefix, _), do: prefix

  # Case conventions, from a snake_case field name, as serde's rename_all.
  @doc false
  def rename(name, nil), do: name
  def rename(name, :snake_case), do: name
  def rename(name, :lowercase), do: name
  def rename(name, :uppercase), do: String.upcase(name)
  def rename(name, :screaming_snake_case), do: String.upcase(name)
  def rename(name, :kebab_case), do: String.replace(name, "_", "-")
  def rename(name, :screaming_kebab_case), do: name |> String.upcase() |> String.replace("_", "-")

  def rename(name, :pascal_case),
    do: name |> String.split("_") |> Enum.map_join(&upcase_first/1)

  def rename(name, :camel_case) do
    case rename(name, :pascal_case) do
      "" -> ""
      <<first::utf8, rest::binary>> -> String.downcase(<<first::utf8>>) <> rest
    end
  end

  defp upcase_first(""), do: ""
  defp upcase_first(<<first::utf8, rest::binary>>), do: String.upcase(<<first::utf8>>) <> rest
end
