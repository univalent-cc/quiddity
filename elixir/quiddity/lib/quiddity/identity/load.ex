defmodule Quiddity.Identity.Load do
  @moduledoc false
  # Identities (spec 5.3): their validation (spec 5.9) and canonical form.
  # Kinds are held in their JSON form: maps with string keys, as JSON gives
  # them.

  alias Quiddity.InvalidIdentity

  @scalars ~w(null bool integer number string uuid path any)
  @compound ~w(bytes instant duration list set tuple map object union tagged ref)
  @units ~w(s ms us ns)
  @source_types ~w(null boolean number string array object bytes set instant duration)
  @min_int -Integer.pow(2, 63)
  @max_int Integer.pow(2, 64) - 1

  defp invalid!(path, message), do: raise(InvalidIdentity.at(path, message))

  @doc "The canonical form of an identity's parts, checked: {domain, root, definitions}."
  def load!(doc) do
    members!(doc, "$", ["root"], ["domain", "definitions"])

    domain = doc["domain"]
    if Map.has_key?(doc, "domain"), do: nonempty_string!(domain, "$.domain")

    defs = Map.get(doc, "definitions", %{})
    unless is_map(defs), do: invalid!("$.definitions", "expected an object")

    defs =
      Map.new(defs, fn {name, kind} ->
        if name == "", do: invalid!("$.definitions", "a definition's name is empty")
        {name, canonical!(kind, "$.definitions.#{name}")}
      end)

    build!(canonical!(doc["root"], "$.root"), defs, domain)
  end

  @doc "Checks canonical parts as a whole, and keeps the definitions the root reaches."
  def build!(root, defs, domain) do
    for {kind, path} <- [
          {root, "$.root"} | Enum.map(defs, fn {n, k} -> {k, "$.definitions.#{n}"} end)
        ] do
      walk!(kind, defs, path)
    end

    reached = reach(root, defs, MapSet.new())
    {domain, root, Map.filter(defs, fn {name, _} -> MapSet.member?(reached, name) end)}
  end

  # --- Syntax and canonical form ----------------------------------------------

  def kind_name!(k, _path) when is_binary(k) and k in @scalars, do: k
  def kind_name!(k, path) when is_binary(k), do: invalid!(path, "unknown kind #{inspect(k)}")

  def kind_name!(%{} = k, path) when map_size(k) == 1 do
    [name] = Map.keys(k)
    if name in @compound, do: name, else: invalid!(path, "unknown kind #{inspect(name)}")
  end

  def kind_name!(_k, path), do: invalid!(path, "a kind is a string or an object with one member")

  defp members!(obj, path, required, optional) do
    unless is_map(obj), do: invalid!(path, "expected an object")

    for m <- required,
        not Map.has_key?(obj, m),
        do: invalid!(path, "missing member #{inspect(m)}")

    for m <- Map.keys(obj),
        m not in required and m not in optional,
        do: invalid!(path, "unknown member #{inspect(m)}")

    :ok
  end

  defp one_of!(value, allowed, path) do
    unless value in allowed,
      do: invalid!(path, "#{inspect(value)} is not one of #{inspect(allowed)}")
  end

  defp nonempty_string!(value, path) do
    unless is_binary(value) and value != "", do: invalid!(path, "expected a non-empty string")
  end

  defp integer?(x), do: is_integer(x)

  def canonical!(k, path) do
    name = kind_name!(k, path)
    if is_binary(k), do: k, else: canonical_compound!(name, k[name], path)
  end

  defp canonical_compound!("bytes", v, path) do
    members!(v, path, [], ["text"])
    if Map.has_key?(v, "text"), do: one_of!(v["text"], ["base64", "hex"], path)
    %{"bytes" => if(v["text"] == "hex", do: %{"text" => "hex"}, else: %{})}
  end

  defp canonical_compound!(name, v, path) when name in ["instant", "duration"] do
    members!(v, path, [], ["unit"])

    if Map.has_key?(v, "unit") do
      one_of!(v["unit"], @units, path)
      %{name => %{"unit" => v["unit"]}}
    else
      %{name => %{}}
    end
  end

  defp canonical_compound!(name, v, path) when name in ["list", "set"],
    do: %{name => canonical!(v, "#{path}.#{name}")}

  defp canonical_compound!("tuple", v, path) do
    unless is_list(v) and v != [], do: invalid!(path, "a tuple needs a non-empty array of kinds")

    %{
      "tuple" =>
        v |> Enum.with_index() |> Enum.map(fn {x, i} -> canonical!(x, "#{path}.tuple[#{i}]") end)
    }
  end

  defp canonical_compound!("map", v, path) do
    members!(v, path, ["key", "value"], [])

    %{
      "map" => %{
        "key" => canonical!(v["key"], "#{path}.key"),
        "value" => canonical!(v["value"], "#{path}.value")
      }
    }
  end

  defp canonical_compound!("object", v, path), do: %{"object" => canonical_object!(v, path)}

  defp canonical_compound!("union", v, path) do
    unless is_list(v) and v != [], do: invalid!(path, "a union needs a non-empty array of kinds")

    alts =
      v
      |> Enum.with_index()
      |> Enum.flat_map(fn {x, i} ->
        case canonical!(x, "#{path}.union[#{i}]") do
          %{"union" => inner} -> inner
          c -> [c]
        end
      end)
      |> Enum.uniq()
      |> Enum.sort_by(&order_key/1)

    case alts do
      [one] -> one
      many -> %{"union" => many}
    end
  end

  defp canonical_compound!("tagged", v, path), do: %{"tagged" => canonical_tagged!(v, path)}

  defp canonical_compound!("ref", v, path) do
    nonempty_string!(v, path)
    %{"ref" => v}
  end

  # A fixed order for union alternatives: by their encodings.
  defp order_key(kind), do: kind |> Quiddity.Value.from() |> Quiddity.Value.encode()

  defp canonical_object!(v, path) do
    members!(v, path, ["fields"], ["unlisted", "order"])
    unless is_map(v["fields"]), do: invalid!(path, "fields must be an object")

    {fields, _keys} =
      Enum.reduce(Enum.sort(v["fields"]), {%{}, MapSet.new()}, fn {fname, f}, {acc, keys} ->
        fpath = "#{path}.fields.#{fname}"
        field = canonical_field!(fname, f, fpath)
        key = Map.get(field, "rename", fname)

        if MapSet.member?(keys, key),
          do: invalid!(fpath, "two fields have the key #{inspect(key)}")

        {Map.put(acc, fname, field), MapSet.put(keys, key)}
      end)

    if Map.has_key?(v, "unlisted"), do: one_of!(v["unlisted"], ["skip"], "#{path}.unlisted")

    case Map.fetch(v, "order") do
      :error ->
        %{"fields" => fields}

      {:ok, order} ->
        unless is_list(order) and Enum.sort(order) == Enum.sort(Map.keys(fields)) and
                 length(Enum.uniq(order)) == length(order),
               do: invalid!(path, "order must name every field exactly once")

        if Enum.any?(Map.values(fields), &Map.has_key?(&1, "omit")),
          do: invalid!(path, "a field with omit in an object with order")

        %{"fields" => fields, "order" => order}
    end
  end

  defp canonical_field!(fname, f, path) do
    members!(f, path, ["kind"], ["rename", "omit", "quantize", "decimals"])
    out = %{"kind" => canonical!(f["kind"], "#{path}.kind")}

    out =
      case Map.fetch(f, "rename") do
        {:ok, r} ->
          nonempty_string!(r, path)
          if r != fname, do: Map.put(out, "rename", r), else: out

        :error ->
          out
      end

    out =
      case Map.fetch(f, "omit") do
        {:ok, omit} ->
          unless is_list(omit) and length(Enum.uniq(omit)) == length(omit),
            do: invalid!(path, "omit must be an array of distinct conditions")

          Enum.each(omit, &one_of!(&1, ["none", "empty"], path))
          if omit != [], do: Map.put(out, "omit", Enum.sort(omit)), else: out

        :error ->
          out
      end

    if Map.has_key?(f, "quantize") and Map.has_key?(f, "decimals"),
      do: invalid!(path, "a field has quantize or decimals, not both")

    out =
      case Map.fetch(f, "quantize") do
        {:ok, q} ->
          unless integer?(q) and q >= 1, do: invalid!(path, "quantize must be a positive integer")
          if q != 1, do: Map.put(out, "quantize", q), else: out

        :error ->
          out
      end

    case Map.fetch(f, "decimals") do
      {:ok, d} ->
        unless integer?(d) and d >= -300 and d <= 300,
          do: invalid!(path, "decimals must be an integer from -300 to 300")

        Map.put(out, "decimals", d)

      :error ->
        out
    end
  end

  defp canonical_tagged!(v, path) do
    members!(v, path, ["variants"], ["style", "tag", "content", "encode_as"])
    style = Map.get(v, "style", "external")
    one_of!(style, ["external", "internal", "adjacent"], path)
    encode_as = Map.get(v, "encode_as", "string")
    one_of!(encode_as, ["string", "int"], path)

    if style == "external" and (Map.has_key?(v, "tag") or Map.has_key?(v, "content")),
      do: invalid!(path, "tag and content are not allowed with external")

    if style in ["internal", "adjacent"] do
      unless Map.has_key?(v, "tag"), do: invalid!(path, "#{style} needs tag")
      nonempty_string!(v["tag"], path)
    end

    if style == "internal" and Map.has_key?(v, "content"),
      do: invalid!(path, "content is not allowed with internal")

    if style == "adjacent" do
      unless Map.has_key?(v, "content"), do: invalid!(path, "adjacent needs content")
      nonempty_string!(v["content"], path)
      if v["content"] == v["tag"], do: invalid!(path, "content must differ from tag")
    end

    variants = v["variants"]

    unless is_map(variants) and map_size(variants) > 0,
      do: invalid!(path, "variants must be a non-empty object")

    {out_variants, _labels} =
      Enum.reduce(Enum.sort(variants), {%{}, MapSet.new()}, fn {vname, var}, {acc, labels} ->
        vpath = "#{path}.variants.#{vname}"
        members!(var, vpath, [], ["kind", "rename", "discriminant"])

        out =
          if Map.has_key?(var, "kind"), do: %{"kind" => canonical!(var["kind"], vpath)}, else: %{}

        out =
          case Map.fetch(var, "rename") do
            {:ok, r} ->
              nonempty_string!(r, vpath)
              if r != vname, do: Map.put(out, "rename", r), else: out

            :error ->
              out
          end

        {out, label} =
          if encode_as == "int" do
            d = var["discriminant"]
            unless integer?(d), do: invalid!(vpath, "encode_as int needs an integer discriminant")

            unless d >= @min_int and d <= @max_int,
              do: invalid!(vpath, "a discriminant is a 64-bit integer")

            {Map.put(out, "discriminant", d), {:int, d}}
          else
            if Map.has_key?(var, "discriminant"),
              do: invalid!(vpath, "discriminant is allowed only with encode_as int")

            {out, {:str, Map.get(out, "rename", vname)}}
          end

        if MapSet.member?(labels, label),
          do: invalid!(vpath, "two variants have the label #{inspect(elem(label, 1))}")

        {Map.put(acc, vname, out), MapSet.put(labels, label)}
      end)

    out = %{"variants" => out_variants}

    out =
      if style != "external",
        do: Map.merge(out, %{"style" => style, "tag" => v["tag"]}),
        else: out

    out = if style == "adjacent", do: Map.put(out, "content", v["content"]), else: out
    if encode_as != "string", do: Map.put(out, "encode_as", encode_as), else: out
  end

  # --- Checks that need the whole identity (spec 5.9) -------------------------

  def reach(k, _defs, reached) when is_binary(k), do: reached

  def reach(%{"ref" => name}, defs, reached) do
    if MapSet.member?(reached, name),
      do: reached,
      else: reach(Map.fetch!(defs, name), defs, MapSet.put(reached, name))
  end

  def reach(%{"list" => k}, defs, r), do: reach(k, defs, r)
  def reach(%{"set" => k}, defs, r), do: reach(k, defs, r)
  def reach(%{"tuple" => ks}, defs, r), do: Enum.reduce(ks, r, &reach(&1, defs, &2))
  def reach(%{"union" => ks}, defs, r), do: Enum.reduce(ks, r, &reach(&1, defs, &2))
  def reach(%{"map" => m}, defs, r), do: reach(m["value"], defs, reach(m["key"], defs, r))

  def reach(%{"object" => o}, defs, r),
    do: o["fields"] |> Map.values() |> Enum.reduce(r, &reach(&1["kind"], defs, &2))

  def reach(%{"tagged" => t}, defs, r) do
    t["variants"]
    |> Map.values()
    |> Enum.reduce(r, fn var, acc ->
      if Map.has_key?(var, "kind"), do: reach(var["kind"], defs, acc), else: acc
    end)
  end

  def reach(_k, _defs, r), do: r

  defp walk!(k, _defs, _path) when is_binary(k), do: :ok

  defp walk!(%{"ref" => name} = k, defs, path) do
    unless Map.has_key?(defs, name), do: invalid!(path, "reference to undefined #{inspect(name)}")
    claims!(k, defs, [], path)
    :ok
  end

  defp walk!(%{"list" => k}, defs, path), do: walk!(k, defs, path)
  defp walk!(%{"set" => k}, defs, path), do: walk!(k, defs, path)
  defp walk!(%{"tuple" => ks}, defs, path), do: Enum.each(ks, &walk!(&1, defs, path))

  defp walk!(%{"map" => m}, defs, path) do
    walk!(m["key"], defs, path)
    walk!(m["value"], defs, path)

    accepting =
      m["key"]
      |> alternatives(defs)
      |> Enum.filter(fn a ->
        {native, conv} = claims!(a, defs, [], path)
        "string" in native or "string" in conv
      end)

    case accepting do
      [] -> invalid!(path, "a map's key kind must accept strings")
      [_] -> :ok
      _ -> invalid!(path, "a map's key union has two alternatives that accept strings")
    end
  end

  defp walk!(%{"object" => o}, defs, path) do
    for {fname, f} <- o["fields"] do
      walk!(f["kind"], defs, path)

      for {option, kinds} <- [
            {"quantize", ~w(integer instant duration)},
            {"decimals", ~w(number)}
          ],
          Map.has_key?(f, option),
          quantized_kind(f["kind"], defs, path) not in kinds do
        invalid!(path, "field #{inspect(fname)}: #{option} does not apply to its kind")
      end
    end

    :ok
  end

  defp walk!(%{"union" => alts}, defs, path) do
    Enum.each(alts, &walk!(&1, defs, path))
    union_claims!(alts, defs, [], path)
    :ok
  end

  defp walk!(%{"tagged" => t}, defs, path) do
    for {_n, var} <- t["variants"], Map.has_key?(var, "kind"), do: walk!(var["kind"], defs, path)

    if t["style"] == "internal" do
      for {vname, var} <- t["variants"], Map.has_key?(var, "kind") do
        case resolve(var["kind"], defs, path) do
          %{"object" => o} ->
            if Map.has_key?(o, "order"),
              do: invalid!(path, "internally tagged variant #{inspect(vname)} has order")

            tag = t["tag"]

            for {fname, f} <- o["fields"],
                tag in [fname, Map.get(f, "rename", fname)],
                do: invalid!(path, "variant #{inspect(vname)} has a field named #{inspect(tag)}")

          _ ->
            invalid!(path, "internally tagged variant #{inspect(vname)} is not an object")
        end
      end
    end

    :ok
  end

  defp walk!(_k, _defs, _path), do: :ok

  @doc "The name of the kind a quantization applies to."
  def quantized_kind(k, defs, path) do
    k = resolve(k, defs, path)

    k =
      case k do
        %{"union" => alts} ->
          case Enum.reject(alts, &(resolve(&1, defs, path) == "null")) do
            [one] -> resolve(one, defs, path)
            _ -> nil
          end

        k ->
          k
      end

    if k == nil, do: nil, else: kind_name!(k, path)
  end

  @doc "A kind with references followed."
  def resolve(k, defs, path, seen \\ [])

  def resolve(%{"ref" => name}, defs, path, seen) do
    if name in seen, do: invalid!(path, "a cycle of references through #{inspect(name)}")
    resolve(Map.fetch!(defs, name), defs, path, [name | seen])
  end

  def resolve(k, _defs, _path, _seen), do: k

  @doc "A kind's union alternatives, or the kind itself."
  def alternatives(k, defs) do
    case resolve(k, defs, "$") do
      %{"union" => alts} -> alts
      _ -> [k]
    end
  end

  @doc "The source types a kind accepts: {natively, by conversion}."
  def claims!(k, defs, visiting \\ [], path \\ "$")

  def claims!(%{"ref" => name}, defs, visiting, path) do
    if name in visiting,
      do: invalid!(path, "a cycle of references through #{inspect(name)} without structure")

    claims!(Map.fetch!(defs, name), defs, [name | visiting], path)
  end

  def claims!(%{"union" => alts}, defs, visiting, path),
    do: union_claims!(alts, defs, visiting, path)

  def claims!(k, _defs, _visiting, path) do
    case kind_name!(k, path) do
      "null" ->
        {["null"], []}

      "bool" ->
        {["boolean"], ["string"]}

      "integer" ->
        {["number"], ["string"]}

      "number" ->
        {["number"], ["string"]}

      "string" ->
        {["string"], []}

      "uuid" ->
        {["string"], []}

      "path" ->
        {["string"], ["object"]}

      "any" ->
        {@source_types, []}

      "list" ->
        {["array"], []}

      "set" ->
        {["array", "set"], []}

      "tuple" ->
        {["array"], []}

      "map" ->
        {["object"], []}

      "object" ->
        {["object"], []}

      "bytes" ->
        {["bytes"], ["array", "string"]}

      name when name in ["instant", "duration"] ->
        number = if Map.has_key?(k[name], "unit"), do: ["number"], else: []
        {[name], ["string", "object"] ++ number}

      "tagged" ->
        t = k["tagged"]

        if Map.get(t, "style", "external") != "external" do
          {["object"], []}
        else
          native =
            t["variants"]
            |> Map.values()
            |> Enum.map(&if(Map.has_key?(&1, "kind"), do: "object", else: "string"))
            |> Enum.uniq()

          {native, []}
        end
    end
  end

  defp union_claims!(alts, defs, visiting, path) do
    # Each alternative's claims first: they detect cycles of references,
    # which looking for object alternatives would follow forever.
    {native, conv} =
      Enum.reduce(alts, {[], []}, fn a, {native, conv} ->
        {n, c} = claims!(a, defs, visiting, path)
        overlap = intersect(native, n)

        overlap =
          if overlap == ["object"] and object_alternatives(alts, defs) != nil,
            do: [],
            else: overlap

        if overlap != [],
          do: invalid!(path, "two union alternatives accept #{inspect(overlap)} natively")

        if intersect(conv, c) != [],
          do:
            invalid!(
              path,
              "two union alternatives accept #{inspect(intersect(conv, c))} by conversion"
            )

        {Enum.uniq(native ++ n), Enum.uniq(conv ++ c)}
      end)

    objects = object_alternatives(alts, defs)

    if objects != nil and length(objects) > 1 do
      for {a, i} <- Enum.with_index(objects), b <- Enum.drop(objects, i + 1) do
        if subset?(required(a, defs), Map.keys(b["fields"])) and
             subset?(required(b, defs), Map.keys(a["fields"])) do
          invalid!(
            path,
            "two object alternatives of a union could both fit one object: " <>
              "#{inspect(Enum.sort(Map.keys(a["fields"])))} and #{inspect(Enum.sort(Map.keys(b["fields"])))}"
          )
        end
      end
    end

    {native, conv}
  end

  defp intersect(a, b), do: Enum.filter(a, &(&1 in b))
  defp subset?(a, b), do: Enum.all?(a, &(&1 in b))

  @doc "The object kinds among a union's alternatives, if every alternative accepting objects is one; else nil."
  def object_alternatives(alts, defs) do
    Enum.reduce_while(alts, [], fn a, acc ->
      case resolve(a, defs, "$") do
        %{"object" => o} ->
          {:cont, acc ++ [o]}

        _ ->
          {native, _} = claims!(a, defs)
          if "object" in native, do: {:halt, nil}, else: {:cont, acc}
      end
    end)
  end

  @doc "The fields of an object kind whose kinds do not accept null."
  def required(o, defs) do
    for {name, f} <- o["fields"],
        {native, _} = claims!(f["kind"], defs),
        "null" not in native,
        do: name
  end
end
