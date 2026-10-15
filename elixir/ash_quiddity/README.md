# AshQuiddity

[Quiddity](https://hexdocs.pm/quiddity) digests for
[Ash](https://hexdocs.pm/ash) resources. A resource's definition is its
identity: which fields a digest covers, under which names, read as their
types say. The digest and the resource cannot drift apart.

## Installation

```elixir
def deps do
  [
    {:ash_quiddity, "~> 0.1"}
  ]
end
```

## Usage

Add the extension to a resource:

```elixir
defmodule Helpdesk.Ticket do
  use Ash.Resource, extensions: [AshQuiddity.Resource]

  attributes do
    uuid_primary_key :id
    attribute :subject, :string, public?: true
    attribute :priority, :integer, public?: true
    attribute :tags, {:array, :string}, public?: true
    timestamps(public?: true)
  end

  quiddity do
    domain "helpdesk.ticket"
    field :id, skip: true
    field :tags, encode_as: :set
    field :updated_at, skip: true
  end
end
```

Then digest its records with Quiddity:

```elixir
Quiddity.digest!(ticket)
#=> #Quiddity.Digest<sha256:…>
```

By default, a digest covers the resource's public attributes that are not
sensitive, under their names. Calculations, aggregates and relationships
are left out unless a `field` includes them, since they are loaded on
demand. A field that a digest covers but that is not loaded, or that a
field policy hides, is an error, never an omission. Everything else can be
changed per field: `skip`, `include`, `rename`, `omit_nil`, `omit_empty`,
`quantize`, `decimals`, `encode_as: :set`, `flatten`.

### More than one identity

```elixir
quiddity do
  identity :meaning do
    field :priority, skip: true
  end
end

Quiddity.digest!(ticket, :meaning)
```

### Following AshJsonApi

With `follow :ash_json_api`, a digest covers what AshJsonApi exports of a
record, under the names it exports them (`default_fields`, `show_fields`,
`hide_fields`, `field_names`, `include_nil_values?`), and the primary key.

### A digest as a calculation

```elixir
calculations do
  calculate :etag, :string, {AshQuiddity.Calculation, form: :micro}
end
```

The digest is computed only when the calculation is loaded, and Ash first
loads the fields it covers.

### The identity as data

`Quiddity.identity(Helpdesk.Ticket)` exports the resource's identity,
which other implementations of quiddity apply to a record's JSON form.

See `AshQuiddity` and `AshQuiddity.Resource` for the details.

## Examples

The [`examples`](https://github.com/univalent-cc/quiddity/tree/main/elixir/ash_quiddity/examples)
directory has scripts to run with `mix run examples/<name>.exs`.

## License

MIT.
