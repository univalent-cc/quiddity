defmodule AshQuiddity.Resource do
  @field_schema [
    name: [
      type: :atom,
      required: true,
      doc: "The attribute, calculation, aggregate or relationship."
    ],
    skip: [type: :boolean, doc: "Leave the field out."],
    include: [
      type: :boolean,
      doc:
        "Digest the field although the default rules leave it out (a calculation, an aggregate, a relationship, or an attribute that is not public or is sensitive)."
    ],
    rename: [type: :string, doc: "The key it is digested under."],
    omit_nil: [type: :boolean, doc: "Leave it out when it is `nil`."],
    omit_empty: [
      type: :boolean,
      doc: "Leave it out when it is an empty string, list, map or set."
    ],
    quantize: [
      type: :pos_integer,
      doc:
        "Round an integer, a `DateTime` or a duration down to a multiple of this step (nanoseconds for times)."
    ],
    decimals: [type: :integer, doc: "Round a float down to this many decimal places."],
    encode_as: [type: {:in, [:set]}, doc: "`:set`: the order of a list's items does not count."],
    flatten: [
      type: :boolean,
      doc: "Write the members of a map or an embedded resource into the containing record's."
    ],
    identity: [type: :atom, doc: "The identity a nested record is digested under."]
  ]

  @field %Spark.Dsl.Entity{
    name: :field,
    describe: "Settings for one field.",
    examples: ["field :tags, encode_as: :set", "field :inserted_at, skip: true"],
    target: AshQuiddity.Dsl.Field,
    args: [:name],
    schema: @field_schema
  }

  @selection_schema [
    follow: [
      type: {:in, [:core, :ash_json_api]},
      doc:
        "Whose rules decide the fields and their names: Ash's own (`:core`, the public attributes that are not sensitive), or AshJsonApi's (`:ash_json_api`)."
    ],
    only: [type: {:list, :atom}, doc: "Digest only these fields."],
    except: [type: {:list, :atom}, doc: "Leave these fields out."]
  ]

  @identity %Spark.Dsl.Entity{
    name: :identity,
    describe: """
    An additional identity: another choice of the fields a digest covers.
    Its settings replace the section's for it: its `only` or `except`
    replaces the field selection, and its `field` settings are merged over
    the section's, option by option.
    """,
    examples: [
      """
      identity :meaning do
        field :priority, skip: true
      end
      """
    ],
    target: AshQuiddity.Dsl.Identity,
    args: [:name],
    schema:
      [name: [type: :atom, required: true, doc: "The identity's name."]] ++ @selection_schema,
    entities: [fields: [@field]]
  }

  @quiddity %Spark.Dsl.Section{
    name: :quiddity,
    describe: """
    How records of the resource are digested by `Quiddity.digest/2`.

    By default, a digest covers the resource's public attributes that are
    not sensitive, under their names. Everything can be changed per field.
    """,
    examples: [
      """
      quiddity do
        domain "helpdesk.ticket"
        field :tags, encode_as: :set
        field :updated_at, skip: true
      end
      """
    ],
    schema:
      [
        domain: [
          type: :string,
          doc: "A label hashed into every digest of the resource's records (spec 4.2)."
        ]
      ] ++ @selection_schema,
    entities: [@field, @identity]
  }

  @moduledoc """
  The Ash extension that makes records of a resource digestible with
  `Quiddity.digest/2`.

      defmodule Helpdesk.Ticket do
        use Ash.Resource, extensions: [AshQuiddity.Resource]

        attributes do
          uuid_primary_key :id
          attribute :subject, :string, public?: true
          attribute :tags, {:array, :string}, public?: true
          timestamps(public?: true)
        end

        quiddity do
          domain "helpdesk.ticket"
          field :tags, encode_as: :set
          field :updated_at, skip: true
        end
      end

  See `AshQuiddity` for the rules.

  <!--- ash-hq-hide-start--> <!--- -->

  ## DSL Documentation

  ### Index

  #{Spark.Dsl.Extension.doc_index([@quiddity])}

  ### Docs

  #{Spark.Dsl.Extension.doc([@quiddity])}
  <!--- ash-hq-hide-stop--> <!--- -->
  """

  use Spark.Dsl.Extension,
    sections: [@quiddity],
    transformers: [AshQuiddity.Transformers.AddEncoder],
    verifiers: [AshQuiddity.Verifiers.VerifyFields]
end
