defprotocol Quiddity.Encoder do
  @moduledoc """
  How a struct is digested: the data-model value (`Quiddity.Value`) it
  stands for, under an identity.

  Derive it to digest a struct's fields, as `JSON.Encoder` and Jason are
  derived:

      defmodule MyApp.Task do
        @derive {Quiddity.Encoder, except: [:submitted_at]}
        defstruct [:prompt, :priority, :submitted_at]
      end

  ## Options

  ### On the struct

  | Option | Meaning |
  |---|---|
  | `only: [...]`, `except: [...]` | The fields digested, as `JSON.Encoder`'s derive selects them. By default, every field. |
  | `domain: "label"` | A label hashed into every digest of the struct, so that its digests differ from those of other types with the same fields (spec 4.2). |
  | `rename_all: rule` | A case convention for every field's key, as serde and Ash name them: `:camel_case`, `:pascal_case`, `:snake_case`, `:kebab_case`, `:screaming_snake_case`, `:screaming_kebab_case`, `:lowercase` or `:uppercase`. A field's own `rename` takes precedence. |
  | `fields: [name: [...]]` | Settings for single fields, by field name (below). |
  | `identities: [name: [...]]` | Additional identities, by name, each with `only`, `except` and `fields`. An identity's `only` or `except` replaces the field selection for it, and its `fields` settings are merged over the others, option by option. See `Quiddity.digest/2`. |

  ### On a field, under `fields:`

  | Option | Meaning |
  |---|---|
  | `skip: true` | Leave the field out. |
  | `rename: "key"` | Digest it under another key. |
  | `omit_nil: true` | Leave it out when it is `nil`, so that a field added later does not change the digests of existing values. |
  | `omit_empty: true` | Leave it out when it is an empty string, list, map or set. |
  | `quantize: n` | Round an integer, a `DateTime` or a duration down to a multiple of `n`, in nanoseconds for times (`1_000_000_000` is to the second). |
  | `decimals: d` | Round a float down to `d` decimal places (to hundreds with `d: -2`), taking it as the shortest decimal that reads back as it. |
  | `encode_as: :set` | Digest a list as a set: the order of its items does not count. Repeated items are kept. |
  | `flatten: true` | Write the members of a map or a struct into the containing struct's map. A key written twice is an error. |
  | `identity: name` | Digest a nested struct under its identity `name`. |
  | `kind: kind` | The field's kind in the struct's exported identity (`Quiddity.identity/2`). By default, the kind the struct's `@type t` typespec gives the field, or `:any` without one. |

  ## Implementing it by hand

  `encode/2` returns the value's data-model form, usually built with
  `Quiddity.Value.from/1`:

      defimpl Quiddity.Encoder, for: Interval do
        def encode(%Interval{start: start, length: length}, _identity) do
          Quiddity.Value.from(%{"start" => start, "end" => start + length})
        end
      end
  """

  @doc """
  The data-model value of `value` under the identity named `identity`
  (`:primary` for the primary identity).
  """
  @spec encode(t, atom) :: Quiddity.Value.t()
  def encode(value, identity)

  # `@derive {Quiddity.Encoder, opts}`: the options are checked and resolved
  # when the struct is compiled. The generated implementation calls shared
  # code, and the struct's module gets its configuration as __quiddity__/1,
  # for domains and for exporting its identity.
  @doc false
  defmacro __deriving__(module, opts) do
    fields =
      module
      |> Macro.struct_info!(__CALLER__)
      |> Enum.map(& &1.field)

    config = Quiddity.Derive.config!(module, fields, opts)

    quote do
      @doc false
      def __quiddity__(:config), do: unquote(Macro.escape(config))
      def __quiddity__({:domain, _identity}), do: unquote(config.domain)
      def __quiddity__({:identity, name}), do: Quiddity.Derive.export!(__MODULE__, name)

      defimpl Quiddity.Encoder, for: unquote(module) do
        @config unquote(Macro.escape(config))

        def encode(value, identity), do: Quiddity.Derive.encode(value, identity, @config)
      end
    end
  end
end
