defmodule Quiddity.Digest do
  @moduledoc """
  A digest: 32 bytes of SHA-256 (spec section 4), or its first 16 or 8
  bytes (`mini/1`, `micro/1`).

  A digest's text form is `sha256:` followed by 64 lowercase hex digits,
  `sha256-128:` and 32 digits, or `sha256-64:` and 16 digits (spec 4.4).
  `to_string/1` writes it, `parse/1` reads it, and digests are encoded to
  JSON as their text.

  ## Examples

      iex> digest = Quiddity.digest!("Summarize the report")
      iex> digest |> Quiddity.Digest.micro() |> to_string()
      "sha256-64:366d2ab7f2c3e7db"
  """

  @enforce_keys [:bytes]
  defstruct [:bytes]

  @typedoc "A digest: 32, 16 or 8 bytes."
  @type t :: %__MODULE__{bytes: <<_::256>> | <<_::128>> | <<_::64>>}

  @domain_prefix "quiddity/1/domain"

  @doc """
  The first 128 bits of a digest.
  """
  @spec mini(t) :: t
  def mini(%__MODULE__{bytes: <<head::binary-16, _::binary-16>>}), do: %__MODULE__{bytes: head}

  @doc """
  The first 64 bits of a digest.

  Enough to compare two known values; not enough for large collections or
  where collisions could be forced.
  """
  @spec micro(t) :: t
  def micro(%__MODULE__{bytes: <<head::binary-8, _::binary-24>>}), do: %__MODULE__{bytes: head}

  @doc """
  The digest `digest` in the domain `label` (spec 4.2), so that it differs
  from the digest of the same value in any other domain.

  ## Examples

      iex> d = Quiddity.digest!(%{"to" => "bob", "amount" => 100})
      iex> Quiddity.Digest.in_domain(d, "example.payment") != d
      true
  """
  @spec in_domain(t, String.t()) :: t
  def in_domain(%__MODULE__{bytes: <<_::binary-32>> = bytes}, label)
      when is_binary(label) and label != "" do
    hash = :crypto.hash(:sha256, [@domain_prefix, label, <<byte_size(label)::64>>, bytes])
    %__MODULE__{bytes: hash}
  end

  def in_domain(%__MODULE__{}, label) when is_binary(label) and label != "",
    do: raise(ArgumentError, "only a full 256-bit digest can be put in a domain")

  def in_domain(%__MODULE__{}, _label),
    do: raise(ArgumentError, "a domain label is a non-empty binary")

  @doc """
  Reads a digest's text form.

  The prefix is lowercase; the hex digits may be in either case.

  ## Examples

      iex> {:ok, d} = Quiddity.Digest.parse("sha256-64:366d2ab7f2c3e7db")
      iex> byte_size(d.bytes)
      8

      iex> Quiddity.Digest.parse("sha256:abc")
      :error
  """
  @spec parse(String.t()) :: {:ok, t} | :error
  def parse("sha256:" <> hex) when byte_size(hex) == 64, do: from_hex(hex)
  def parse("sha256-128:" <> hex) when byte_size(hex) == 32, do: from_hex(hex)
  def parse("sha256-64:" <> hex) when byte_size(hex) == 16, do: from_hex(hex)
  def parse(text) when is_binary(text), do: :error

  @doc """
  Reads a digest's text form, raising `ArgumentError` if it is not one.
  """
  @spec parse!(String.t()) :: t
  def parse!(text) do
    case parse(text) do
      {:ok, d} -> d
      :error -> raise ArgumentError, "not a digest: #{inspect(text)}"
    end
  end

  defp from_hex(hex) do
    case Base.decode16(hex, case: :mixed) do
      {:ok, bytes} -> {:ok, %__MODULE__{bytes: bytes}}
      :error -> :error
    end
  end

  @doc false
  def prefix(%__MODULE__{bytes: bytes}) do
    case byte_size(bytes) do
      32 -> "sha256:"
      16 -> "sha256-128:"
      8 -> "sha256-64:"
    end
  end

  defimpl String.Chars do
    def to_string(digest),
      do: Quiddity.Digest.prefix(digest) <> Base.encode16(digest.bytes, case: :lower)
  end

  defimpl Inspect do
    def inspect(digest, _opts), do: "#Quiddity.Digest<#{digest}>"
  end

  defimpl JSON.Encoder do
    def encode(digest, encoder), do: JSON.Encoder.BitString.encode(to_string(digest), encoder)
  end

  if Code.ensure_loaded?(Jason.Encoder) do
    defimpl Jason.Encoder do
      def encode(digest, opts), do: Jason.Encode.string(to_string(digest), opts)
    end
  end
end
