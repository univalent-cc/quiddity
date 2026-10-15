defmodule Quiddity.Value do
  @moduledoc """
  Values of quiddity's data model (spec section 2), and their encoding
  (spec section 3).

  A digest is the hash of a value's encoding. Elixir terms are first read
  as values of the data model with `from/1`; implementations of
  `Quiddity.Encoder` return values in this form.

  ## The data model

  | Value | Data model |
  |---|---|
  | `nil` | null |
  | `true`, `false` | boolean |
  | an integer | integer, of any size |
  | a float | number; a float with an integer value is that integer (spec 2.2) |
  | `{:float, :nan}`, `{:float, :infinity}`, `{:float, :neg_infinity}` | the floats the BEAM cannot represent |
  | a binary | string: its bytes, text or not |
  | `{:list, values}` | list, in order |
  | `{:set, values}` | set: order does not count, repeated members do |
  | `{:map, [{key, value}]}` | map: order does not count |
  | `{:instant, nanoseconds}` | an instant, encoded as its integer nanoseconds since the Unix epoch (spec 2.6) |
  | `{:duration, nanoseconds}` | a duration, encoded as its integer nanoseconds |

  Instants and durations encode as integers. They are kept apart from
  integers so that an identity's `instant` and `duration` kinds can accept
  them as they are.
  """

  import Bitwise, only: [band: 2]

  @typedoc "A value of the data model."
  @type t ::
          nil
          | boolean
          | integer
          | float
          | {:float, :nan | :infinity | :neg_infinity}
          | binary
          | {:list, [t]}
          | {:set, [t]}
          | {:map, [{t, t}]}
          | {:instant, integer}
          | {:duration, integer}

  @t_null 0x01
  @t_false 0x02
  @t_true 0x03
  @t_int 0x10
  @t_float 0x11
  @t_string 0x20
  @t_list 0x30
  @t_set 0x31
  @t_map 0x32

  @doc """
  Reads an Elixir term as a value of the data model.

  Values are read as Elixir's JSON encoders write them, except where the
  data model can say more than JSON:

    * atoms other than `nil`, `true` and `false` are strings of their
      names, and map keys that are atoms are strings too;
    * `Date`, `Time`, `NaiveDateTime` and `Duration` are their ISO 8601
      text, and a `Decimal` its decimal text, without an exponent;
    * a `DateTime` is an instant, and a `MapSet` a set;
    * a `Quiddity.Digest` is the string of its text form (spec 6.2);
    * a struct is read through its `Quiddity.Encoder` implementation.

  Tuples (and so keyword lists), a map with both `:a` and `"a"` as keys,
  and structs that do not implement `Quiddity.Encoder` are errors, raised
  as `Quiddity.Error`.

  ## Examples

      iex> Quiddity.Value.from(%{name: "Ada", tags: [:math]})
      {:map, [{"name", "Ada"}, {"tags", {:list, ["math"]}}]}

      iex> Quiddity.Value.from(1.0)
      1

      iex> Quiddity.Value.from(MapSet.new([1]))
      {:set, [1]}
  """
  @spec from(term) :: t
  def from(term), do: from(term, "$")

  @doc false
  def from(nil, _path), do: nil
  def from(true, _path), do: true
  def from(false, _path), do: false
  def from(n, _path) when is_integer(n), do: n
  def from(x, _path) when is_float(x), do: normalize(x)
  def from(s, _path) when is_binary(s), do: s
  def from(a, _path) when is_atom(a), do: Atom.to_string(a)

  def from(list, path) when is_list(list) do
    {:list, list_items(list, path, 0)}
  end

  def from(%MapSet{} = set, path) do
    {:set, set |> MapSet.to_list() |> Enum.map(&from(&1, path <> "[]"))}
  end

  # A digest used as a value is the string of its text form (spec 6.2).
  def from(%Quiddity.Digest{} = d, _path), do: to_string(d)
  def from(%DateTime{} = dt, _path), do: {:instant, DateTime.to_unix(dt, :nanosecond)}
  def from(%NaiveDateTime{} = dt, _path), do: NaiveDateTime.to_iso8601(dt)
  def from(%Date{} = d, _path), do: Date.to_iso8601(d)
  def from(%Time{} = t, _path), do: Time.to_iso8601(t)
  def from(%Duration{} = d, _path), do: Duration.to_iso8601(d)

  def from(%{__struct__: Decimal} = d, _path) do
    # As Jason writes it; see history/ELIXIR.md, P5.
    apply(Decimal, :to_string, [d, :normal])
  end

  def from(%{__struct__: module} = struct, path) do
    if Quiddity.Encoder.impl_for(struct) do
      try do
        Quiddity.Encoder.encode(struct, :primary)
      rescue
        e in Quiddity.Error -> reraise %{e | path: nest(path, e.path)}, __STACKTRACE__
      end
    else
      raise Quiddity.Error.at(
              path,
              "#{inspect(module)} does not implement Quiddity.Encoder; derive it with " <>
                "@derive Quiddity.Encoder, or implement it"
            )
    end
  end

  def from(%{} = map, path) do
    entries =
      Enum.map(map, fn {k, v} ->
        key = key(k, path)
        {key, from(v, path <> "." <> to_string(key_text(key)))}
      end)

    check_keys(entries, path)
    {:map, entries}
  end

  def from(t, path) when is_tuple(t) do
    raise Quiddity.Error.at(
            path,
            "a tuple has no data-model value (a keyword list is a list of tuples); " <>
              "use a list or a map"
          )
  end

  def from(other, path) do
    raise Quiddity.Error.at(path, "#{inspect(other)} has no data-model value")
  end

  defp nest(prefix, "$" <> rest), do: prefix <> rest
  defp nest(prefix, _), do: prefix

  defp list_items([], _path, _i), do: []

  defp list_items([x | rest], path, i),
    do: [from(x, "#{path}[#{i}]") | list_items(rest, path, i + 1)]

  defp list_items(_improper, path, _i),
    do: raise(Quiddity.Error.at(path, "an improper list has no data-model value"))

  defp key(k, _path) when is_atom(k) and k not in [nil, true, false], do: Atom.to_string(k)
  defp key(k, path), do: from(k, path)

  defp key_text(k) when is_binary(k), do: k
  defp key_text(k), do: inspect(k)

  defp check_keys(entries, path) do
    entries
    |> Enum.map(&elem(&1, 0))
    |> Enum.frequencies()
    |> Enum.find(fn {_k, n} -> n > 1 end)
    |> case do
      nil ->
        :ok

      {k, _} ->
        raise Quiddity.Error.at(
                path,
                "the key #{inspect(k)} is given twice (as an atom and as a string); " <>
                  "its JSON form would repeat it"
              )
    end
  end

  @doc """
  A float as the data model holds it (spec 2.2): a finite float with an
  integer value is that integer.

  ## Examples

      iex> Quiddity.Value.normalize(-0.0)
      0

      iex> Quiddity.Value.normalize(1.5)
      1.5
  """
  @spec normalize(float) :: float | integer
  def normalize(x) when is_float(x) do
    if x == Float.floor(x), do: trunc(x), else: x
  end

  @doc """
  The encoding of a value (spec section 3), the bytes a digest hashes.

  ## Examples

      iex> Quiddity.Value.encode(1) |> Base.encode16(case: :lower)
      "01000000000000000110"

      iex> Quiddity.Value.encode({:list, [true]}) |> Base.encode16(case: :lower)
      "03000000000000000130"
  """
  @spec encode(t) :: binary
  def encode(value), do: value |> encode_iodata() |> IO.iodata_to_binary()

  defp encode_iodata(nil), do: <<@t_null>>
  defp encode_iodata(false), do: <<@t_false>>
  defp encode_iodata(true), do: <<@t_true>>
  defp encode_iodata(n) when is_integer(n), do: int(n)
  defp encode_iodata({:instant, n}) when is_integer(n), do: int(n)
  defp encode_iodata({:duration, n}) when is_integer(n), do: int(n)

  defp encode_iodata(x) when is_float(x) do
    case normalize(x) do
      n when is_integer(n) -> int(n)
      x -> <<x::float-big-64, @t_float>>
    end
  end

  defp encode_iodata({:float, :nan}), do: <<0x7FF8000000000000::64, @t_float>>
  defp encode_iodata({:float, :infinity}), do: <<0x7FF0000000000000::64, @t_float>>
  defp encode_iodata({:float, :neg_infinity}), do: <<0xFFF0000000000000::64, @t_float>>

  defp encode_iodata(s) when is_binary(s), do: [s, <<byte_size(s)::64, @t_string>>]

  defp encode_iodata({:list, items}),
    do: [Enum.map(items, &encode_iodata/1), <<length(items)::64, @t_list>>]

  defp encode_iodata({:set, members}) do
    sorted = members |> Enum.map(&encode/1) |> Enum.sort()
    [sorted, <<length(sorted)::64, @t_set>>]
  end

  defp encode_iodata({:map, entries}) do
    sorted =
      entries
      |> Enum.map(fn {k, v} -> {encode(k), encode(v)} end)
      |> Enum.sort()
      |> Enum.map(fn {k, v} -> [k, v] end)

    [sorted, <<length(sorted)::64, @t_map>>]
  end

  defp encode_iodata(other) do
    raise ArgumentError, "not a value of the data model: #{inspect(other)}"
  end

  # Minimal two's-complement big-endian bytes, then the length and tag
  # (spec 3.4).
  defp int(n) do
    magnitude = if n >= 0, do: n, else: -n - 1
    bytes = :binary.encode_unsigned(magnitude)

    size =
      if band(:binary.first(bytes), 0x80) != 0, do: byte_size(bytes) + 1, else: byte_size(bytes)

    <<n::signed-big-size(size * 8), size::64, @t_int>>
  end
end
