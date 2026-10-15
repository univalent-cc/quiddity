defmodule Quiddity.Identity.Apply do
  @moduledoc false
  # Applying an identity to a source value (spec 5.4 to 5.8). Sources are
  # data-model values (Quiddity.Value), read as JSON gives them: an object
  # is a {:map, entries} with string keys.

  alias Quiddity.Identity.{Load, Text}
  alias Quiddity.{Quantize, Value}

  @units %{"s" => 1_000_000_000, "ms" => 1_000_000, "us" => 1_000, "ns" => 1}
  @max_digits 4096

  defp fail!(path, message), do: raise(Quiddity.Error.at(path, message))

  def apply!(root, defs, source), do: read(root, source, defs, "$")

  defp source_type(nil), do: "null"
  defp source_type(b) when is_boolean(b), do: "boolean"
  defp source_type(n) when is_number(n), do: "number"
  defp source_type({:float, _}), do: "number"
  defp source_type(s) when is_binary(s), do: "string"
  defp source_type({:list, _}), do: "array"
  defp source_type({:map, _}), do: "object"
  defp source_type({:set, _}), do: "set"
  defp source_type({:instant, _}), do: "instant"
  defp source_type({:duration, _}), do: "duration"

  defp claims(k, defs), do: Load.claims!(k, defs)
  defp resolve(k, defs), do: Load.resolve(k, defs, "$")

  defp read(%{"ref" => name}, s, defs, path), do: read(Map.fetch!(defs, name), s, defs, path)

  defp read(%{"union" => alts}, s, defs, path) do
    t = source_type(s)
    objects = Enum.filter(alts, &match?(%{"object" => _}, resolve(&1, defs)))

    if t == "object" and length(objects) > 1 do
      members = object_members(s)

      fit =
        Enum.find(objects, fn a ->
          %{"object" => o} = resolve(a, defs)
          names = Map.keys(members)

          Enum.all?(names, &Map.has_key?(o["fields"], &1)) and
            Enum.all?(Load.required(o, defs), &(&1 in names))
        end)

      cond do
        fit != nil ->
          read(fit, s, defs, path)

        conv = Enum.find(alts, &("object" in elem(claims(&1, defs), 1))) ->
          read(conv, s, defs, path)

        true ->
          fail!(path, "the object fits none of the union's object alternatives")
      end
    else
      alt =
        Enum.find(alts, &(t in elem(claims(&1, defs), 0))) ||
          Enum.find(alts, &(t in elem(claims(&1, defs), 1)))

      if alt,
        do: read(alt, s, defs, path),
        else: fail!(path, "no alternative of the union accepts a #{t}")
    end
  end

  defp read(k, s, defs, path) do
    name = Load.kind_name!(k, path)
    t = source_type(s)
    {native, conv} = claims(k, defs)
    unless t in native or t in conv, do: fail!(path, "#{name} does not accept a #{t}")
    read_kind(name, k, s, t, defs, path)
  end

  defp read_kind("any", _k, s, _t, _defs, _path), do: s
  defp read_kind("null", _k, _s, _t, _defs, _path), do: nil
  defp read_kind("bool", _k, s, "boolean", _defs, _path), do: s
  defp read_kind("bool", _k, s, "string", _defs, path), do: ok!(Text.bool(s), path)

  defp read_kind("integer", _k, s, "number", _defs, path) do
    if is_integer(s), do: s, else: fail!(path, "not an integer: #{inspect(s)}")
  end

  defp read_kind("integer", _k, s, "string", _defs, path) do
    case Text.decimal(Text.trim(s)) do
      {_m, e} when e > @max_digits -> fail!(path, "out of range: #{inspect(s)}")
      _ -> ok!(Text.integer(s), path)
    end
  end

  defp read_kind("number", _k, s, "number", _defs, _path), do: s
  defp read_kind("number", _k, s, "string", _defs, path), do: ok!(Text.number(s), path)
  defp read_kind("string", _k, s, _t, _defs, _path), do: s
  defp read_kind("uuid", _k, s, _t, _defs, path), do: ok!(Text.uuid(s), path)
  defp read_kind("path", _k, s, "string", _defs, _path), do: s

  defp read_kind("path", _k, s, "object", _defs, path) do
    case object_members(s) do
      %{"Unix" => units} = m when map_size(m) == 1 ->
        path_units(units, 255, path) |> :binary.list_to_bin()

      %{"Windows" => units} = m when map_size(m) == 1 ->
        path_units(units, 65_535, path) |> Text.wtf8()

      _ ->
        fail!(path, "a path object has exactly one member, Unix or Windows")
    end
  end

  defp read_kind("bytes", _k, {:list, items}, _t, _defs, path) do
    if Enum.all?(items, &(is_integer(&1) and &1 >= 0 and &1 <= 255)),
      do: :binary.list_to_bin(items),
      else: fail!(path, "bytes must be integers from 0 to 255")
  end

  defp read_kind("bytes", k, s, "string", _defs, path) do
    if k["bytes"]["text"] == "hex", do: ok!(Text.hex(s), path), else: ok!(Text.base64(s), path)
  end

  defp read_kind(name, k, s, t, _defs, path) when name in ["instant", "duration"],
    do: {String.to_atom(name), time(s, t, k[name]["unit"], name, path)}

  defp read_kind("list", k, s, _t, defs, path), do: {:list, items(k["list"], s, defs, path)}
  defp read_kind("set", k, s, _t, defs, path), do: {:set, items(k["set"], s, defs, path)}

  defp read_kind("tuple", k, {:list, items}, _t, defs, path) do
    kinds = k["tuple"]

    if length(kinds) != length(items),
      do: fail!(path, "expected #{length(kinds)} items, found #{length(items)}")

    {:list,
     Enum.zip(kinds, items)
     |> Enum.with_index()
     |> Enum.map(fn {{kk, x}, i} -> read(kk, x, defs, "#{path}[#{i}]") end)}
  end

  defp read_kind("map", k, {:map, entries}, _t, defs, path) do
    %{"key" => key_kind, "value" => value_kind} = k["map"]

    {:map,
     Enum.map(entries, fn {key, x} ->
       p = "#{path}.#{key_text(key)}"
       {read(key_kind, key, defs, p), read(value_kind, x, defs, p)}
     end)}
  end

  defp read_kind("object", k, s, _t, defs, path), do: object(k["object"], s, defs, nil, path)
  defp read_kind("tagged", k, s, _t, defs, path), do: tagged(k["tagged"], s, defs, path)

  defp items(kind, {_list_or_set, xs}, defs, path),
    do: xs |> Enum.with_index() |> Enum.map(fn {x, i} -> read(kind, x, defs, "#{path}[#{i}]") end)

  defp path_units({:list, units}, limit, path) do
    if Enum.all?(units, &(is_integer(&1) and &1 >= 0 and &1 <= limit)),
      do: units,
      else: fail!(path, "a path's units are integers from 0 to #{limit}")
  end

  defp path_units(_, limit, path),
    do: fail!(path, "a path's units are integers from 0 to #{limit}")

  defp key_text(k) when is_binary(k), do: k
  defp key_text(k), do: inspect(k)

  defp ok!({:ok, v}, _path), do: v
  defp ok!({:error, message}, path), do: fail!(path, message)

  # The members of a source object, by name.
  defp object_members({:map, entries}) do
    Map.new(entries, fn
      {k, v} when is_binary(k) -> {k, v}
      {k, v} -> {k, v}
    end)
  end

  # --- Instants and durations (spec 5.5) ---------------------------------------

  defp time({_tag, ns}, t, _unit, name, _path) when t == name, do: ns

  defp time(s, "number", unit, name, path) do
    if unit == nil, do: fail!(path, "a number for #{name} needs a declared unit")
    exact_nanos(ratio(s), @units[unit], s, unit, path)
  end

  defp time(s, "string", unit, name, path) do
    text = Text.trim(s)

    case {unit, Text.decimal(text)} do
      {unit, {m, e}} when unit != nil ->
        exact_nanos(decimal_ratio(m, e, path), @units[unit], s, unit, path)

      _ ->
        if name == "instant",
          do: ok!(Text.instant(text), path),
          else: ok!(Text.duration(text), path)
    end
  end

  defp time(s, "object", _unit, name, path) do
    names =
      if name == "instant",
        do: ["nanos_since_epoch", "secs_since_epoch"],
        else: ["nanos", "secs"]

    members = object_members(s)

    if Enum.sort(Map.keys(members)) != names,
      do: fail!(path, "expected exactly the members #{inspect(Enum.reverse(names))}")

    [nanos_name, secs_name] = names
    secs = members[secs_name]
    nanos = members[nanos_name]

    unless is_integer(secs) and is_integer(nanos),
      do: fail!(path, "#{secs_name} and #{nanos_name} must be integers")

    if secs < 0 or nanos < 0 or nanos >= 1_000_000_000,
      do: fail!(path, "#{secs_name} and #{nanos_name} out of range")

    secs * 1_000_000_000 + nanos
  end

  defp time(_s, t, _unit, name, path), do: fail!(path, "#{name} does not accept a #{t}")

  # A number as an exact ratio {numerator, denominator}.
  defp ratio(n) when is_integer(n), do: {n, 1}

  defp ratio(x) when is_float(x) do
    <<sign::1, exp::11, frac::52>> = <<x::float-64>>

    {m, e2} =
      if exp == 0, do: {frac, -1074}, else: {frac + Integer.pow(2, 52), exp - 1075}

    m = if sign == 1, do: -m, else: m
    if e2 >= 0, do: {m * Integer.pow(2, e2), 1}, else: {m, Integer.pow(2, -e2)}
  end

  defp ratio(other), do: fail!("$", "not a finite number: #{inspect(other)}")

  defp decimal_ratio(m, e, path) do
    cond do
      e > @max_digits -> fail!(path, "out of range")
      e >= 0 -> {m * Integer.pow(10, e), 1}
      true -> {m, Integer.pow(10, -e)}
    end
  end

  defp exact_nanos({num, den}, scale, s, unit, path) do
    n = num * scale

    if rem(n, den) == 0,
      do: div(n, den),
      else: fail!(path, "#{inspect(s)} #{unit} is not a whole number of nanoseconds")
  end

  # --- Objects and tagged unions (spec 5.6, 5.8) -------------------------------

  defp object(o, s, defs, skip, path) do
    members = object_members(s)

    # Not a comprehension: its `v = ...` filters would drop nil and false.
    values =
      o["fields"]
      |> Enum.sort()
      |> Enum.flat_map(fn {fname, f} ->
        member = if fname == skip, do: nil, else: members[fname]
        v = read(f["kind"], member, defs, "#{path}.#{fname}") |> quantize(f)

        if omitted?(v, Map.get(f, "omit", [])),
          do: [],
          else: [{fname, Map.get(f, "rename", fname), v}]
      end)

    case o do
      %{"order" => order} ->
        by_name = Map.new(values, fn {n, _, v} -> {n, v} end)
        {:list, Enum.map(order, &Map.get(by_name, &1))}

      _ ->
        {:map, Enum.map(values, fn {_, key, v} -> {key, v} end)}
    end
  end

  defp quantize(v, %{"quantize" => step}), do: Quantize.quantize(v, step)
  defp quantize(v, %{"decimals" => d}), do: Quantize.decimals(v, d)
  defp quantize(v, _f), do: v

  defp omitted?(v, omit) do
    ("none" in omit and v == nil) or ("empty" in omit and empty?(v))
  end

  defp empty?(""), do: true
  defp empty?({tag, []}) when tag in [:list, :set, :map], do: true
  defp empty?(_), do: false

  defp tagged(t, s, defs, path) do
    style = Map.get(t, "style", "external")
    int? = t["encode_as"] == "int"
    variants = t["variants"]

    label = fn vname ->
      var = variants[vname]
      if int?, do: var["discriminant"], else: Map.get(var, "rename", vname)
    end

    variant = fn vname ->
      if is_binary(vname) and Map.has_key?(variants, vname),
        do: variants[vname],
        else: fail!(path, "unknown variant #{inspect(vname)}")
    end

    if style == "external" do
      case s do
        name when is_binary(name) ->
          if Map.has_key?(variant.(name), "kind"),
            do: fail!(path, "variant #{inspect(name)} has a value")

          label.(name)

        {:map, [{vname, x}]} ->
          var = variant.(vname)

          unless Map.has_key?(var, "kind"),
            do: fail!(path, "unit variant #{inspect(vname)} has a value")

          v = read(var["kind"], x, defs, "#{path}.#{vname}")
          if int?, do: {:list, [label.(vname), v]}, else: {:map, [{label.(vname), v}]}

        _ ->
          fail!(path, "an externally tagged value is a string or an object with one member")
      end
    else
      tag = t["tag"]
      members = if match?({:map, _}, s), do: object_members(s), else: %{}

      unless match?({:map, _}, s) and Map.has_key?(members, tag),
        do: fail!(path, "expected an object with the member #{inspect(tag)}")

      vname = members[tag]
      var = variant.(vname)
      tag_entry = {tag, label.(vname)}

      cond do
        not Map.has_key?(var, "kind") ->
          {:map, [tag_entry]}

        style == "internal" ->
          %{"object" => o} = resolve(var["kind"], defs)

          {:map, entries} =
            object(o, {:map, Enum.reject(elem(s, 1), &(elem(&1, 0) == tag))}, defs, nil, path)

          {:map, entries ++ [tag_entry]}

        true ->
          content = t["content"]

          unless Map.has_key?(members, content),
            do: fail!(path, "expected the member #{inspect(content)}")

          {:map,
           [tag_entry, {content, read(var["kind"], members[content], defs, "#{path}.#{content}")}]}
      end
    end
  end

  @doc false
  def value_from_source(term), do: Value.from(term)
end
