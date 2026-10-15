defmodule Quiddity.Quantize do
  @moduledoc false
  # Quantization (spec 5.6): a value rounded down to a grid, exactly.

  alias Quiddity.Value

  @doc "`value` rounded down to a multiple of `step`: integers, instants and durations."
  def quantize(nil, _step), do: nil
  def quantize(n, step) when is_integer(n), do: Integer.floor_div(n, step) * step
  def quantize({:instant, n}, step), do: {:instant, quantize(n, step)}
  def quantize({:duration, n}, step), do: {:duration, quantize(n, step)}

  def quantize(other, _step),
    do: raise(ArgumentError, "quantize applies to integers and times, not #{inspect(other)}")

  @doc """
  `value` rounded down to `d` decimal places. A float is taken as the
  shortest decimal that reads back as it; the result is an integer when
  its value is one.
  """
  def decimals(nil, _d), do: nil
  def decimals({:float, _} = special, _d), do: special

  def decimals(n, d) when is_integer(n) do
    if d >= 0, do: n, else: floor_scaled(n, 0, d)
  end

  def decimals(x, d) when is_float(x) do
    {mantissa, exponent} = shortest_decimal(x)
    floor_scaled(mantissa, exponent, d)
  end

  def decimals(other, _d),
    do: raise(ArgumentError, "decimals applies to numbers, not #{inspect(other)}")

  # mantissa × 10^exponent, rounded down to a multiple of 10^-d, as a
  # data-model number.
  defp floor_scaled(mantissa, exponent, d) do
    shift = exponent + d

    n =
      if shift >= 0,
        do: mantissa * pow10(shift),
        else: Integer.floor_div(mantissa, pow10(-shift))

    cond do
      d <= 0 -> n * pow10(-d)
      rem(n, pow10(d)) == 0 -> div(n, pow10(d))
      true -> Value.normalize(:erlang.binary_to_float("#{n}.0e-#{d}"))
    end
  end

  # The shortest decimal that reads back as `x`, as {mantissa, exponent}.
  defp shortest_decimal(x) do
    text = :erlang.float_to_binary(x, [:short])

    {digits, exponent} =
      case String.split(text, ["e", "E"]) do
        [digits, exp] -> {digits, String.to_integer(exp)}
        [digits] -> {digits, 0}
      end

    case String.split(digits, ".") do
      [int, frac] -> {String.to_integer(int <> frac), exponent - byte_size(frac)}
      [int] -> {String.to_integer(int), exponent}
    end
  end

  defp pow10(n), do: Integer.pow(10, n)
end
