defmodule Quiddity.Identity.Text do
  @moduledoc false
  # The conversions of spec 5.5: values written as text, read as the kind
  # declares. Each function returns {:ok, value} or {:error, message}.

  import Bitwise

  @min_int -Integer.pow(2, 63)
  @max_int Integer.pow(2, 64) - 1

  # Spec 5.5 removes ASCII whitespace only, from both ends.
  def trim(s), do: Regex.replace(~r/\A[ \t\n\r]+|[ \t\n\r]+\z/, s, "")

  @bools %{
    "true" => true,
    "t" => true,
    "yes" => true,
    "y" => true,
    "on" => true,
    "1" => true,
    "false" => false,
    "f" => false,
    "no" => false,
    "n" => false,
    "off" => false,
    "0" => false
  }

  def bool(s) do
    case Map.fetch(@bools, s |> trim() |> String.downcase()) do
      {:ok, b} -> {:ok, b}
      :error -> {:error, "not a boolean: #{inspect(s)}"}
    end
  end

  @decimal ~r/\A([+-]?)(0|[1-9][0-9]*)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?\z/

  @doc "Decimal text as an exact {mantissa, exponent}: mantissa × 10^exponent."
  def decimal(text) do
    case Regex.run(@decimal, text, capture: :all_but_first) do
      nil ->
        nil

      [sign, int | rest] ->
        frac = Enum.at(rest, 0, "")
        exp = Enum.at(rest, 1, "")
        exp = if exp == "", do: 0, else: String.to_integer(exp)
        m = String.to_integer(int <> frac)
        m = if sign == "-", do: -m, else: m
        {m, exp - byte_size(frac)}
    end
  end

  @doc "The value spec 2.7 gives decimal text as a JSON number."
  def json_number(text) do
    body = String.trim_leading(text, "+")

    with true <- Regex.match?(~r/\A-?(0|[1-9][0-9]*)\z/, body),
         n = String.to_integer(body),
         true <- n >= @min_int and n <= @max_int do
      n
    else
      _ ->
        {m, e} = decimal(text)
        to_float(m, e)
    end
  end

  @doc "The binary64 value nearest to m × 10^e, normalized."
  def to_float(0, _e), do: 0

  def to_float(m, e) do
    Quiddity.Value.normalize(:erlang.binary_to_float("#{m}.0e#{e}"))
  rescue
    ArgumentError ->
      cond do
        e < 0 -> 0
        m > 0 -> {:float, :infinity}
        true -> {:float, :neg_infinity}
      end
  end

  @number_tokens %{
    "nan" => :nan,
    "+nan" => :nan,
    "-nan" => :nan,
    "inf" => :infinity,
    "+inf" => :infinity,
    "infinity" => :infinity,
    "+infinity" => :infinity,
    "∞" => :infinity,
    "+∞" => :infinity,
    "-inf" => :neg_infinity,
    "-infinity" => :neg_infinity,
    "-∞" => :neg_infinity
  }

  def number(s) do
    text = trim(s)
    token = text |> String.downcase() |> String.replace("−", "-")

    cond do
      Map.has_key?(@number_tokens, token) -> {:ok, {:float, @number_tokens[token]}}
      decimal(text) != nil -> {:ok, json_number(text)}
      true -> {:error, "not a number: #{inspect(s)}"}
    end
  end

  def integer(s) do
    case decimal(trim(s)) do
      {m, e} when e >= 0 ->
        {:ok, m * Integer.pow(10, e)}

      {m, e} ->
        d = Integer.pow(10, -e)
        if rem(m, d) == 0, do: {:ok, div(m, d)}, else: {:error, "not an integer: #{inspect(s)}"}

      nil ->
        {:error, "not an integer: #{inspect(s)}"}
    end
  end

  @uuid ~r/\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z/
  @uuid_simple ~r/\A[0-9a-f]{32}\z/

  def uuid(s) do
    text = s |> trim() |> String.downcase()

    text =
      cond do
        String.starts_with?(text, "{") and String.ends_with?(text, "}") ->
          String.slice(text, 1..-2//1)

        String.starts_with?(text, "urn:uuid:") ->
          rest = String.replace_prefix(text, "urn:uuid:", "")
          if Regex.match?(@uuid, rest), do: rest, else: :invalid

        true ->
          text
      end

    text =
      if is_binary(text) and Regex.match?(@uuid_simple, text) do
        <<a::binary-8, b::binary-4, c::binary-4, d::binary-4, e::binary-12>> = text
        Enum.join([a, b, c, d, e], "-")
      else
        text
      end

    if is_binary(text) and Regex.match?(@uuid, text),
      do: {:ok, text},
      else: {:error, "not a UUID: #{inspect(s)}"}
  end

  def hex(s) do
    text = trim(s)

    if Regex.match?(~r/\A(?:[0-9a-fA-F]{2})*\z/, text),
      do: {:ok, Base.decode16!(text, case: :mixed)},
      else: {:error, "not hex: #{inspect(s)}"}
  end

  def base64(s) do
    text = trim(s)
    body = String.trim_trailing(text, "=")
    padding = byte_size(text) - byte_size(body)
    std? = String.contains?(body, ["+", "/"])
    url? = String.contains?(body, ["-", "_"])

    cond do
      std? and url? ->
        {:error, "base64 mixes the standard and URL-safe alphabets"}

      padding > 0 and (rem(byte_size(text), 4) != 0 or padding > 2) ->
        {:error, "incorrect base64 padding"}

      rem(byte_size(body), 4) == 1 ->
        {:error, "not base64"}

      true ->
        body = body |> String.replace("-", "+") |> String.replace("_", "/")

        case Base.decode64(body, padding: false) do
          {:ok, bytes} -> {:ok, bytes}
          :error -> {:error, "not base64: #{inspect(s)}"}
        end
    end
  end

  @rfc3339 ~r/\A(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(?:([Zz])|([+-])(\d{2})(?::?(\d{2}))?)?\z/

  def instant(text) do
    case Regex.run(@rfc3339, text) do
      nil ->
        {:error, "not an RFC 3339 date-time: #{inspect(text)}"}

      [_ | groups] ->
        groups = groups ++ List.duplicate("", 11 - length(groups))
        [y, mo, d, h, mi, s, frac, _z, sign, oh, om] = groups
        [y, mo, d, h, mi, s] = Enum.map([y, mo, d, h, mi, s], &String.to_integer/1)

        # A second of 60 is a leap second, counted as the next minute's first (spec 2.6).
        cond do
          not (mo in 1..12 and d >= 1 and d <= days_in_month(y, mo) and h <= 23 and mi <= 59 and
                   s <= 60) ->
            {:error, "date or time out of range: #{inspect(text)}"}

          true ->
            offset_result =
              if sign == "" do
                {:ok, 0}
              else
                oh = String.to_integer(oh)
                om = if om == "", do: 0, else: String.to_integer(om)

                if oh > 23 or om > 59,
                  do: {:error, "offset out of range: #{inspect(text)}"},
                  else: {:ok, (oh * 3600 + om * 60) * if(sign == "-", do: -1, else: 1)}
              end

            with {:ok, offset} <- offset_result do
              secs = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s - offset
              {:ok, secs * 1_000_000_000 + frac_nanos(frac)}
            end
        end
    end
  end

  @duration ~r/\A([+-])?[Pp](?:(\d+)[Ww])?(?:(\d+)[Dd])?(?:[Tt](?:(\d+)[Hh])?(?:(\d+)[Mm])?(?:(\d+)(?:\.(\d{1,9}))?[Ss])?)?\z/

  def duration(text) do
    case Regex.run(@duration, text) do
      nil ->
        bad_duration(text)

      [_ | groups] ->
        groups = groups ++ List.duplicate("", 7 - length(groups))
        [sign, w, d, h, mi, s, frac] = groups

        if Enum.all?([w, d, h, mi, s], &(&1 == "")) or String.ends_with?(text, ["T", "t"]) do
          bad_duration(text)
        else
          [w, d, h, mi, s] =
            Enum.map([w, d, h, mi, s], &if(&1 == "", do: 0, else: String.to_integer(&1)))

          secs = (w * 7 + d) * 86400 + h * 3600 + mi * 60 + s
          ns = secs * 1_000_000_000 + frac_nanos(frac)
          {:ok, if(sign == "-", do: -ns, else: ns)}
        end
    end
  end

  defp bad_duration(text),
    do: {:error, "not an ISO 8601 duration without years or months: #{inspect(text)}"}

  defp frac_nanos(""), do: 0
  defp frac_nanos(frac), do: String.to_integer(String.pad_trailing(frac, 9, "0"))

  defp days_from_civil(y, m, d) do
    y = if m <= 2, do: y - 1, else: y
    era = Integer.floor_div(y, 400)
    yoe = y - era * 400
    doy = div(153 * (m + if(m > 2, do: -3, else: 9)) + 2, 5) + d - 1
    doe = yoe * 365 + div(yoe, 4) - div(yoe, 100) + doy
    era * 146_097 + doe - 719_468
  end

  defp days_in_month(y, 2),
    do: if((rem(y, 4) == 0 and rem(y, 100) != 0) or rem(y, 400) == 0, do: 29, else: 28)

  defp days_in_month(_y, m) when m in [4, 6, 9, 11], do: 30
  defp days_in_month(_y, _m), do: 31

  @doc "UTF-16 code units as WTF-8 bytes (spec 2.3): pairs combined, lone surrogates kept."
  def wtf8(units), do: units |> combine() |> Enum.map(&code_point/1) |> IO.iodata_to_binary()

  defp combine([hi, lo | rest]) when hi in 0xD800..0xDBFF and lo in 0xDC00..0xDFFF,
    do: [0x10000 + ((hi - 0xD800) <<< 10) + (lo - 0xDC00) | combine(rest)]

  defp combine([u | rest]), do: [u | combine(rest)]
  defp combine([]), do: []

  # UTF-8's scheme for any code point, surrogates included.
  def code_point(c) when c < 0x80, do: <<c>>
  def code_point(c) when c < 0x800, do: <<0xC0 ||| c >>> 6, 0x80 ||| (c &&& 0x3F)>>

  def code_point(c) when c < 0x10000,
    do: <<0xE0 ||| c >>> 12, 0x80 ||| (c >>> 6 &&& 0x3F), 0x80 ||| (c &&& 0x3F)>>

  def code_point(c),
    do:
      <<0xF0 ||| c >>> 18, 0x80 ||| (c >>> 12 &&& 0x3F), 0x80 ||| (c >>> 6 &&& 0x3F),
        0x80 ||| (c &&& 0x3F)>>
end
