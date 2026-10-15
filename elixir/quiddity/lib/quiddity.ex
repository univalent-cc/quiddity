defmodule Quiddity do
  @moduledoc """
  Stable digests of values, from the fields that make them what they are.

  A digest is the SHA-256 of a value's encoding (see `QUIDDITY_SPEC.md`),
  the same in every language and on every platform. Two values that mean
  the same thing have the same digest: map keys in any order, `1` and
  `1.0`, a `DateTime` in any time zone.

      iex> Quiddity.digest!(%{"a" => 1, "b" => [true, nil]}) ==
      ...>   Quiddity.digest!(%{b: [true, nil], a: 1.0})
      true

  ## Identities

  Which fields count, and how each is read, is an *identity*:

    * a struct's identity comes from its `Quiddity.Encoder` implementation,
      usually derived (`@derive {Quiddity.Encoder, except: [:submitted_at]}`),
      and a struct can declare further named identities;
    * a `Quiddity.Identity` is an identity as data, built in code or read
      from its JSON form, which every implementation of quiddity reads,
      and applies to any value.

  ## Errors

  Functions that can fail return `{:ok, result}` or `{:error, exception}`,
  and their `!` variants raise the exception (`Quiddity.Error`).
  """

  alias Quiddity.{Digest, Identity, Value}

  @value_prefix "quiddity/1/value"

  @doc """
  The digest of `value`, under an identity.

  `identity` is the name of one of a struct's identities (`:primary` by
  default), or a `Quiddity.Identity`, which applies to any value.

  ## Examples

      iex> {:ok, digest} = Quiddity.digest(%{"prompt" => "Summarize the report"})
      iex> to_string(digest)
      "sha256:d438cd022c59365720a55a65784c1d62922c74d2870c86899570f9cbf40c6757"

      iex> Quiddity.digest({:ok, 1})
      {:error, %Quiddity.Error{path: "$", message: "a tuple has no data-model value (a keyword list is a list of tuples); use a list or a map"}}
  """
  @spec digest(term, atom | Identity.t()) :: {:ok, Digest.t()} | {:error, Exception.t()}
  def digest(value, identity \\ :primary) do
    {:ok, digest!(value, identity)}
  rescue
    e in [Quiddity.Error] -> {:error, e}
  end

  @doc """
  The digest of `value`, as `digest/2`, raising `Quiddity.Error` if it has
  none.
  """
  @spec digest!(term, atom | Identity.t()) :: Digest.t()
  def digest!(value, identity \\ :primary)

  def digest!(value, %Identity{} = identity), do: Identity.digest!(identity, value)

  def digest!(%{__struct__: module} = struct, name) when is_atom(name) do
    {value, domain} =
      case Quiddity.Encoder.impl_for(struct) do
        nil ->
          {Value.from(struct), nil}

        _impl ->
          domain =
            if function_exported?(module, :__quiddity__, 1),
              do: module.__quiddity__({:domain, name})

          {Quiddity.Encoder.encode(struct, name), domain}
      end

    value |> hash() |> in_domain(domain)
  end

  def digest!(value, :primary), do: value |> Value.from() |> hash()

  def digest!(value, name) when is_atom(name) do
    raise Quiddity.Error.at(
            "$",
            "#{inspect(value)} is not a struct, so it has no identity #{inspect(name)}"
          )
  end

  @doc """
  The identity of a struct that derives `Quiddity.Encoder`, as data: the
  identity, in its JSON form, that another language can apply to the
  struct's JSON form.

  `name` is one of the struct's identities, `:primary` by default. A
  field's kind is the one its `kind:` option declares; otherwise the one
  the struct's `@type t` typespec gives it (`String.t()` is a string,
  `DateTime.t()` an instant, `t | nil` optional, another struct's `t()`
  its identity, nested); otherwise `:any`. Typespecs are read from the
  compiled module, and are missing where its debug information is stripped,
  as in a default `mix release`; the fields then export as `:any`. Fields
  digested in ways this export cannot express (`flatten`) are an error.

  ## Examples

      iex> defmodule Example.Reading do
      ...>   @derive {Quiddity.Encoder, fields: [price: [kind: :number, decimals: 2]]}
      ...>   defstruct [:price]
      ...> end
      iex> {:ok, id} = Quiddity.identity(Example.Reading)
      iex> Quiddity.Identity.to_json(id)["definitions"]["Reading"]
      %{"object" => %{"fields" => %{"price" => %{"decimals" => 2, "kind" => "number"}}}}
  """
  @spec identity(module, atom) :: {:ok, Identity.t()} | {:error, Exception.t()}
  def identity(module, name \\ :primary) do
    {:ok, identity!(module, name)}
  rescue
    e in [Quiddity.InvalidIdentity, Quiddity.Error] -> {:error, e}
  end

  @doc "The identity of a derived struct, as `identity/2`, raising on error."
  @spec identity!(module, atom) :: Identity.t()
  def identity!(module, name \\ :primary) do
    if Code.ensure_loaded?(module) and function_exported?(module, :__quiddity__, 1),
      do: module.__quiddity__({:identity, name}),
      else: raise(Quiddity.Error.at("$", "#{inspect(module)} does not derive Quiddity.Encoder"))
  end

  @doc false
  def hash(value) do
    %Digest{bytes: :crypto.hash(:sha256, [@value_prefix, Value.encode(value)])}
  end

  defp in_domain(digest, nil), do: digest
  defp in_domain(digest, label), do: Digest.in_domain(digest, label)
end
