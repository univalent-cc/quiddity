defmodule Quiddity.JSON do
  @moduledoc false
  # JSON text read as spec 2.7 maps it: an integer without fraction or
  # exponent within -2^63..2^64-1 is that integer, and any other number the
  # nearest binary64 value; an object with a repeated member name is an
  # error. Elixir's JSON module decodes large integers exactly and keeps the
  # last of repeated names, so OTP's :json decoder is used with these rules.

  @min -Integer.pow(2, 63)
  @max Integer.pow(2, 64) - 1

  @doc "Decodes JSON text; `{:ok, term}` or `{:error, message}`."
  def decode(text) when is_binary(text) do
    decoders = %{
      null: nil,
      integer: &integer/1,
      object_finish: &object_finish/2
    }

    try do
      case :json.decode(text, :ok, decoders) do
        {term, :ok, rest} ->
          if Regex.match?(~r/\A[ \t\n\r]*\z/, rest),
            do: {:ok, term},
            else: {:error, "unexpected text after the JSON value"}
      end
    rescue
      e in ErlangError -> {:error, "not JSON: #{inspect(e.original)}"}
      e in ArgumentError -> {:error, Exception.message(e)}
    catch
      {:duplicate, name} -> {:error, "the member name #{inspect(name)} is repeated"}
    end
  end

  defp integer(text) do
    n = String.to_integer(text)
    if n >= @min and n <= @max, do: n, else: Quiddity.Value.normalize(:erlang.float(n))
  end

  defp object_finish(pairs, old_acc) do
    map = Map.new(pairs)

    if map_size(map) != length(pairs) do
      {name, _} =
        pairs
        |> Enum.frequencies_by(&elem(&1, 0))
        |> Enum.find(fn {_, n} -> n > 1 end)

      throw({:duplicate, name})
    end

    {map, old_acc}
  end
end
