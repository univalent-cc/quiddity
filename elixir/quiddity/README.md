# Quiddity

*Quiddity* (n.): the essence of a thing; what makes it *what it is*, as
distinct from its incidental properties.

Quiddity computes stable digests of values based on meaning annotations.
A digest is the same in every language that implements quiddity, on every
platform, and in every release.

The encoding is specified in [`QUIDDITY_SPEC.md`](https://github.com/univalent-cc/quiddity/blob/main/QUIDDITY_SPEC.md).  The concepts common to every implementation are explained in the
repository's [README](https://github.com/univalent-cc/quiddity#concepts);
this README covers the Elixir package.

## Installation

Add `quiddity` to your dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:quiddity, "~> 0.1"}
  ]
end
```

It needs Elixir 1.18 or later, and has no required dependencies.

## Digests of values

```elixir
iex> digest = Quiddity.digest!(%{prompt: "Summarize the report", priority: 1})
#Quiddity.Digest<sha256:…>
iex> to_string(digest)
"sha256:…"
```

Values are read as Elixir's JSON encoders write them, so two values that
mean the same thing have the same digest: map keys in any order, atom or
string keys, `1` and `1.0`. A `DateTime` is an instant, the same in any time
zone; a `MapSet` is a set, whose order does not count. What JSON cannot
write, such as a tuple, is an error:

```elixir
iex> Quiddity.digest({:ok, 1})
{:error, %Quiddity.Error{path: "$", message: "a tuple has no data-model value …"}}
```

`Quiddity.digest/2` returns `{:ok, digest}` or `{:error, error}`, and
`Quiddity.digest!/2` raises. A digest prints as `sha256:` and 64 hex digits,
encodes to JSON as that text, and is read back with
`Quiddity.Digest.parse/1`. `Quiddity.Digest.micro/1` gives a 64-bit form,
for an ETag, for example.

## Structs

Derive `Quiddity.Encoder` to digest a struct's fields, as `JSON.Encoder` is
derived:

```elixir
defmodule MyApp.Task do
  @derive {Quiddity.Encoder,
           except: [:submitted_at],
           domain: "example.task",
           fields: [
             tags: [encode_as: :set],
             asked_at: [quantize: 1_000_000_000],
             note: [omit_nil: true]
           ]}
  defstruct [:prompt, :priority, :tags, :asked_at, :note, :submitted_at]
end
```

The digest covers every field except `submitted_at`. The order of `tags`
does not count, `asked_at` is rounded down to the second, and `note` is
left out when it is `nil`, so that adding it did not change the digests of
existing tasks. The domain keeps a task's digests apart from those of any
other type with the same fields.

A struct can have more than one identity, for different purposes:

```elixir
@derive {Quiddity.Encoder,
         identities: [meaning: [fields: [priority: [skip: true]]]]}

Quiddity.digest!(task)            # every field
Quiddity.digest!(task, :meaning)  # every field except the priority
```

### Derive options

The options for a struct and for its fields are listed in
[`Quiddity.Encoder`](https://hexdocs.pm/quiddity/Quiddity.Encoder.html):
field selection, renaming, omitting `nil` and empty values, quantization,
sets, flattening, nested identities, domains and additional identities.

## Domains

Two kinds of values with the same fields have the same digest by default.
When this is not desirable, for example when a digest is signed or used as
an identifier, use a domain. A domain is a label, hashed into the digest,
so values in different domains never share digests.

A struct's domain is set with the derive's `domain:` option, and an
identity's with `Quiddity.Identity.new/2`'s. `Quiddity.Digest.in_domain/2`
puts an existing digest in a domain.

## Quantization

A field's value can be rounded down to a grid before it is digested, so
that small differences, such as a float's last digits or a timestamp's
milliseconds, don't change the digest:

```elixir
defmodule Reading do
  @derive {Quiddity.Encoder,
           fields: [
             price: [decimals: 2],
             at: [quantize: 1_000_000_000]  # to the second
           ]}
  defstruct [:price, :at]
end

a = %Reading{price: 12.989, at: ~U[2024-01-01 12:00:00.750Z]}
b = %Reading{price: 12.98, at: ~U[2024-01-01 12:00:00Z]}
Quiddity.digest!(a) == Quiddity.digest!(b)
#=> true
```

`quantize:` applies to integers, and to `DateTime`s and durations in
nanoseconds. `decimals:` applies to floats, taken as the decimal they are
written as, so `2.67` stays `2.67`. Values are rounded down, so values on
either side of a band's edge have different digests, however close they
are.

## Identities as data

An identity can also be data, built in code or loaded from an identity
document, the JSON form that every implementation of quiddity reads. It
applies to any value, such as a map decoded from JSON:

```elixir
iex> {:ok, identity} =
...>   Quiddity.Identity.from_json(~s({
...>     "quiddity": 1,
...>     "root": {"object": {"fields": {
...>       "prompt": {"kind": "string"},
...>       "priority": {"kind": "integer"},
...>       "asked_at": {"kind": {"instant": {}}}
...>     }}}
...>   }))
iex> a = Quiddity.digest!(%{"prompt" => "p", "priority" => 1, "asked_at" => "2024-01-01T00:00:00Z", "tag" => "x"}, identity)
iex> b = Quiddity.digest!(%{"prompt" => "p", "priority" => "1", "asked_at" => "2024-01-01T01:00:00+01:00"}, identity)
iex> a == b
true
```

Members the identity does not list are left out, and values written in
other forms are converted: the text `"1"` to an integer, RFC 3339 text to
an instant.

The same identity, built in code:

```elixir
Quiddity.Identity.new!({:object, prompt: :string, priority: :integer, asked_at: :instant})
```

`Quiddity.Identity` describes the kinds, building and changing identities,
and nesting them. `Quiddity.Identity.to_json/1` writes the document, and
`Quiddity.identity/2` exports the identity of a derived struct, for other
languages to apply to its JSON form.

## Examples

The [`examples`](https://github.com/univalent-cc/quiddity/tree/main/elixir/quiddity/examples)
directory has scripts to run with `mix run examples/<name>.exs`.

## License

MIT.
