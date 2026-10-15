defmodule AshQuiddity do
  @moduledoc """
  Quiddity digests for Ash resources.

  Add `AshQuiddity.Resource` to a resource's extensions, and its records
  can be digested with `Quiddity.digest/2`:

      defmodule Helpdesk.Ticket do
        use Ash.Resource, extensions: [AshQuiddity.Resource]

        attributes do
          uuid_primary_key :id
          attribute :subject, :string, public?: true
          attribute :priority, :integer, public?: true
          timestamps()
        end
      end

      Quiddity.digest!(ticket)

  The resource's definition is its identity, so the digest and the
  resource cannot drift apart.

  ## Which fields count

  By default, a digest covers the resource's public attributes that are not
  sensitive, under their names. An attribute that is not public, which
  Ash's encoders leave out, is not digested. A sensitive one, which Ash
  redacts, is not digested either: a digest of a low-entropy secret can be
  reversed by guessing.

  Calculations, aggregates and relationships are not digested unless a
  `field` includes them: they are loaded on demand, and a digest must not
  depend on what happens to be loaded. A field that a digest covers but that
  is not loaded, or that a field policy hides from the actor, is an error,
  never an omission.

  Everything is configurable, per field and per identity, in the `quiddity`
  section; see `AshQuiddity.Resource`.

  ## Following AshJsonApi

  With `follow :ash_json_api`, a digest covers what AshJsonApi exports of a
  record, under the names it exports them: its `default_fields` (or the
  public attributes), adjusted by `show_fields` and `hide_fields`, named by
  `field_names`; the primary key, which JSON:API exports as the object's
  `id`; and `nil` values omitted when `include_nil_values?` is false. A
  field's `rename` is then an error: AshJsonApi's `field_names` names the
  fields.

  ## Types

  An attribute's value is digested as its JSON-safe form
  (`Ash.Type.dump_to_embedded/3`), read as Quiddity reads Elixir values: a
  `DateTime` as an instant, a `Date` or a `Decimal` as its text, an atom as
  its name. A record of an embedded resource is digested under its own
  `quiddity` section.

  ## A digest as a field

  `AshQuiddity.Calculation` gives a record's digest as a calculation,
  computed only when loaded.

  ## Identities as data

  `Quiddity.identity/2` exports a resource's identity, for other languages
  to apply to a record's JSON form.
  """
end
