# Elixir port: design

Started 2026-10-14. This document records the design of the Elixir
implementation of quiddity, as `history/DESIGN.md` records the
specification and the Rust crate. Decisions are numbered E1, E2, …; open
questions are collected after them, and provisional implementation
decisions (P1…, A1…) at the end.

Status, 2026-10-15: implemented in `elixir/quiddity` (the core, dynamic
identities, the derive, multiple identities) and `elixir/ash_quiddity`
(the Ash extension, the AshJsonApi adapter, the calculation, export).
Both pass their tests; `quiddity` passes every group of the
language-neutral vectors and agrees with the Rust crate on a composed
identity.

## Context

- **Elixir:** 1.19.5 on OTP 28 is installed. Elixir 1.18 added a built-in
  `JSON` module and a `JSON.Encoder` protocol, derivable with `only` and
  `except`.
- **Ash:** 3.36. An Ash resource is declared with a DSL built on Spark
  (`use Ash.Resource`), with sections such as `attributes`,
  `relationships`, `calculations`, `aggregates` and `identities`. An
  extension (`use Spark.Dsl.Extension`) adds DSL sections and transformers
  to a resource: `use Ash.Resource, extensions: [Some.Extension]`.
  Introspection (`Ash.Resource.Info.attributes/1`,
  `Ash.Resource.Info.public_attributes/1`, …) works on the module and,
  inside transformers, on the DSL state.
- **Ash attributes** have a name, a type and options. The options that
  matter here: `public?` (default `false`), `sensitive?`, `allow_nil?`,
  `primary_key?`, `writable?`, `generated?`, `constraints`, and `source`
  (the data layer's name for the field, not its JSON name). Timestamps and
  generated keys are declared with `create_timestamp`, `update_timestamp`,
  `uuid_primary_key` and `integer_primary_key`.
- **Ash types:** built-in short names include `:string`, `:ci_string`,
  `:integer`, `:float`, `:decimal`, `:boolean`, `:atom`, `:uuid`,
  `:uuid_v7`, `:binary`, `:date`, `:time`, `:utc_datetime`,
  `:utc_datetime_usec`, `:naive_datetime`, `:datetime`, `:duration`,
  `:map`, `:keyword`, `:tuple`, `:struct`, `:term`, `:union`, `:module`,
  `:vector`; arrays are `{:array, type}`; embedded resources are resources
  with `data_layer: :embedded`. A type converts values for storage
  (`dump_to_native/2`) and to a JSON-encodable form
  (`dump_to_embedded/2`, optional).
- **Unions** (`Ash.Type.Union`) hold `%Ash.Union{type:, value:}`. They are
  stored either as `%{"type" => …, "value" => …}` (`storage:
  :type_and_value`, the default) or as the member's map with a tag field
  (`storage: :map_with_tag`, when every member has `tag` and
  `tag_value`).
- **JSON of a resource:** Ash itself has no single wire form. `ash_jason`
  (3.1) implements `Jason.Encoder` for resources: by default it picks the
  public, non-sensitive fields (attributes, relationships, aggregates,
  calculations) with loaded values, and omits `nil`, `Ash.NotLoaded` and
  `Ash.ForbiddenField`; its `jason` section adds `pick`, `compact`,
  `merge`, `rename`, `order` and `customize` steps. AshJsonApi writes the
  JSON:API envelope (`{"data": {"type", "id", "attributes": …}}`), and
  AshGraphql writes camelCase fields.
- **Hex:** the package name `quiddity` is free.
- **AshJsonApi** (1.7.1 in the ULP application,
  `~/Desktop/Takagi_Legal_Project/takagi/ulp_application`) is the framework
  ULP uses for its JSON: its resources use `extensions:
  [AshJsonApi.Resource]` with a `json_api` section, and its corpus JSON
  Schema (`Ulp.LegalKnowledge.KnowledgeBaseJSONSchema`) is built from
  `AshJsonApi.OpenApi.spec/1`, so that the schema follows the resource
  definitions. What AshJsonApi exports of a record, its "attributes":
  - the fields: `default_fields` if set, else the public attributes (and
    public calculations, when loaded); `show_fields` and `hide_fields`
    adjust the set;
  - their names: `field_names`, either `:camelize`, `:dasherize`, a keyword
    list, or a function;
  - `nil` values: included unless `include_nil_values?` (on the resource,
    else on the domain, default `true`) is false;
  - the primary key is the resource object's `id`, outside its attributes;
    relationships are linkage, outside them too;
  - `sensitive?` does not affect export: a public sensitive attribute is
    exported;
  - each Ash type has a JSON Schema type: `:decimal` is a string, `:date`
    a `date` string, `:utc_datetime` and `:naive_datetime` `date-time`
    strings, `:time` a `time` string, `:uuid` a `uuid` string, atoms and
    enums strings (with `enum` for `one_of`), maps with `fields`
    constraints objects, unions `anyOf` their members, embedded resources
    their own schemas.

- **The layering.** Ash core defines resources: attribute names, types,
  `public?`, `sensitive?`, embedded resources, unions, and each type's
  JSON-safe form (`Ash.Type.dump_to_embedded/3`; by default a type's
  `dump_to_native/2`; in Ash 3.33 an atom dumps to its name, a `Duration`
  to ISO 8601 text, and a `Date` or `Decimal` stays a struct that JSON
  encoders write as text). Encoders are built on core, each with its own
  choices: AshJsonApi (the JSON:API envelope, `field_names`,
  `default_fields`, `include_nil_values?`), ash_jason (`pick`, `rename`,
  `compact`), AshGraphql (camelCase). OpenAPI is a description that
  AshJsonApi generates (through OpenApiSpex) of its own output; ULP builds
  its JSON Schema from it. Core has no renaming and no nil policy: those
  belong to each encoder.

## E1. Scope and order of work

1. The core (spec sections 2–4): the data model, the encoding and the
   digest of Elixir values, checked against `test-vectors/`.
2. Dynamic identities (spec section 5): reading and writing them,
   applying them, conversions, quantization, the identity of identities. In a
   language without static types this is the main tool (D19).
3. Plain structs: a protocol and its derive, the counterpart of
   `#[derive(Quiddity)]`.
4. Ash: an extension that reads a resource's definition as its identity,
   the counterpart of `serde_attributes` (D21).

## E2. Packages (Q8)

Two Mix projects in the repository, as the Ash ecosystem packages its
extensions:

- `elixir/quiddity`: the core, the dynamic identities and the derive, with
  no dependencies (SHA-256 is `:crypto.hash/2`, from OTP; JSON is the
  built-in `JSON` module). The minimum Elixir version is 1.18, for
  `JSON`.
- `elixir/ash_quiddity`: the Ash extension, depending on `quiddity` and
  `ash`, with `ash_json_api` optional, for the adapter. Every Ash extension
  in the ULP application (`ash_json_api`, `ash_postgres`, `ash_oban`,
  `ash_archival`, …) is a separate `ash_*` package depending on `ash`
  with a version bound; ash_jason, a community extension, is too. A
  separate package keeps the core free of Ash, and lets the extension
  follow Ash's releases.

The tests are ExUnit, reading `../../test-vectors/quiddity-v1.json` and
skipping when it is absent, as the Rust tests do.

## E3. Elixir values in the data model

The mapping (spec section 2). Where Elixir is ambiguous, the policy is
that of Elixir's JSON encoders (`JSON`, and Jason, which agree): a value
digests as its JSON form would, and what they refuse is an error. The
exceptions are where the data model is richer than JSON (sets, instants,
bytes), as in the Rust crate (Q9). Checked 2026-10-15 against Elixir
1.19's `JSON` and Jason 1.4.

| Elixir | Data model | Note |
|---|---|---|
| `nil` | null | Checked before other atoms. |
| `true`, `false` | boolean | Checked before other atoms. |
| integer | integer | Exact, any size. |
| float | float, normalized by spec 2.2 | The BEAM has no NaN or infinities; they occur only through an identity's text conversion. |
| binary | string | Elixir does not tell text from bytes; spec D17's one string kind covers both. |
| other atom | string of its name | As both encoders write it (`:ok` is `"ok"`). |
| list | list | Charlists are lists of integers. |
| tuple, keyword list | error | Both encoders refuse tuples, so keyword lists too. A derive or an identity can say what a tuple field means. |
| map | map | Atom keys become strings, so `%{a: 1}` equals `%{"a" => 1}`. A map with both `:a` and `"a"` is an error: both encoders write the key twice, which spec 2.7 gives no meaning. |
| `MapSet` | set | Richer than JSON (`JSON` refuses it; Jason writes a list), as `HashSet` is in Rust. |
| `DateTime` | instant, integer nanoseconds | Spec 2.6, as `SystemTime` in Rust. Its JSON text reads to the same instant under an identity's `instant` kind. |
| `NaiveDateTime`, `Date`, `Time` | string, ISO 8601 | As the encoders write them. No time zone, so not an instant; the data model has no date or time-of-day kind. |
| `Duration` (1.17+) | string, ISO 8601 (`"P1MT2S"`) | As `JSON` writes it. Months have no fixed length, so not integer nanoseconds. |
| `Decimal` | string, its decimal text | As both encoders write it (`"1.00"`); see Q11. |
| an Ash record | through `ash_quiddity` (E6) | |
| other structs | through the protocol (E5), else an error | As both encoders refuse structs that do not implement them. |

Every one of these is a default: a derive, an Ash `field`, or an identity
says otherwise for a field.

## E4. Core API

- `Quiddity.digest(value)` and `Quiddity.digest(value, identity)`,
  returning `{:ok, digest}` or `{:error, error}`, and `digest!/1,2`,
  which raise (E10).
- A digest is a `%Quiddity.Digest{}` with its 32 bytes, implementing
  `String.Chars` (`"sha256:…"`), `Inspect`, and `JSON.Encoder`.
  `Quiddity.Digest.parse/1` reads the text form. `mini/1` and `micro/1`
  give the 128- and 64-bit forms.
- Domains: `Quiddity.digest(value, domain: "…")` and
  `Quiddity.Digest.in_domain/2`.

## E5. Plain structs: a protocol and its derive (Q7)

A protocol, `Quiddity.Encoder`, derived as Elixir's `JSON.Encoder` and
Jason's are, with options. The options are the Ash section's (E6), as a
keyword list, so the two read alike and share one table of field
settings:

```elixir
defmodule Task do
  @derive {Quiddity.Encoder,
           domain: "example.task",
           except: [:submitted_at],
           fields: [
             prompt: [rename: "question"],
             note: [omit_nil: true],
             tags: [encode_as: :set],
             asked_at: [quantize: 1_000_000_000]
           ],
           identities: [
             meaning: [except: [:priority]]
           ]}
  defstruct [:prompt, :priority, :note, :tags, :asked_at, :submitted_at]
end
```

- `only` and `except` select fields, as in `JSON.Encoder`'s derive.
- `fields` holds per-field settings: the Rust derive's field attributes,
  as keyword options (`skip`, `rename`, `omit_nil`, `omit_empty`,
  `quantize`, `decimals`, `encode_as`, `flatten`, `identity`).
- `identities` declares additional identities (Q5), as Rust's
  `identities(Meaning)` does, each with the same options, which replace
  the primary's for that identity.
- `rename_all` and `domain` are type-level options.
- The protocol's function returns the value's data-model form, so a
  struct can also implement it by hand, as a manual `Quiddity` impl in
  Rust.
- `Quiddity.identity(Task)` exports the struct's identity, as
  `construct_identity()` does.

Elixir structs have no per-field attributes, so a keyword list keyed by
field name is the idiomatic form (`@derive` with options is how `JSON`
and Jason are configured). A `use Quiddity` macro with `field`
declarations was considered: it would read like the Ash section, but it
adds a second way to configure a struct, where `@derive` is the one
Elixir developers know.

## E6. Ash: the resource as its identity

An extension, built on Ash core, with an optional adapter for AshJsonApi:

```elixir
defmodule Helpdesk.Ticket do
  use Ash.Resource, extensions: [AshJsonApi.Resource, Quiddity.Ash]

  attributes do
    uuid_primary_key :id
    attribute :subject, :string, public?: true
    attribute :priority, :integer, public?: true
    attribute :tags, {:array, :string}, public?: true
    timestamps()
  end

  json_api do
    type "ticket"
    field_names :camelize
  end

  quiddity do
    follow :ash_json_api
    domain "helpdesk.ticket"
    field :tags, encode_as: :set
    field :updated_at, skip: true
  end
end
```

**Target: Ash core.** The extension reads the resource through
`Ash.Resource.Info` and digests values through `Ash.Type.dump_to_embedded/3`,
below every encoder, so it works with AshJsonApi, ash_jason, AshGraphql or
none. As serde's base crate defines a type's data and serde_json one
format of it, Ash core defines a resource's data and AshJsonApi one
format of it.

- **Fields:** the public attributes that are not `sensitive?`. An
  attribute that is not public is not digested (Q4). In Ash, `sensitive?`
  means "do not display": Ash redacts such attributes when inspecting,
  logging and reporting errors, AshAdmin hides them, and ash_jason leaves
  them out. A digest of a low-entropy secret can be reversed by guessing,
  so core's default leaves them out too (Q14).
- **Names:** the attributes' names.
- **`nil`:** digested as null.
- **Values:** each Ash type maps to a kind (E7). Where the data model has
  no kind for a type, the value is digested as its JSON-safe form from
  `dump_to_embedded/3`, written as JSON encoders write it (a `Date` as
  `"2026-10-15"`).
- **Calculations, aggregates and relationships** are not digested by
  default: they are loaded on demand, and a digest must not depend on what
  was loaded (Q2). The `quiddity` section can include them by name; a
  listed one that is not loaded is an error, not an omission.

**Configuration (Q3).** No policy is inferred from field names: timestamps
and keys are attributes like any other, digested if public. Everything is
configurable in the `quiddity` section, per field, with the Rust derive's
field attributes that apply: `skip`, `rename`, `omit_nil`, `omit_empty`,
`quantize`, `decimals`, `encode_as: :set`, `flatten` (for maps and
embedded resources), and including a calculation, an aggregate, a
relationship or a non-public attribute. An Ash extension cannot add
options to Ash's own `attribute` entity, so the per-field settings are
entities in the `quiddity` section (`field :name, opts`), checked against
the resource at compile time. Resource-level settings: `domain`, `follow`,
and additional identities (Q5). Core cannot say most of these, as serde
cannot say `quantize`.

**Adapter: `follow :ash_json_api`.** For resources served through
AshJsonApi, as ULP's are, the digest can follow its export instead of
core's defaults, so the user does not repeat their renames and the digest
and the API cannot drift (the counterpart of `serde_attributes`, D21). It
reads the resource's `json_api` section:

- fields: `default_fields` if set, else the public attributes, adjusted by
  `show_fields` and `hide_fields`; the primary key, which JSON:API exports
  as the object's `id` (Q13); and public `sensitive?` attributes, which
  AshJsonApi exports, so a digest reveals nothing the API does not (Q14);
- calculations and aggregates listed in `default_fields`: the resource
  exports them by default, so they are digested, and one that is not
  loaded is an error, not an omission (Q12); public calculations that
  AshJsonApi shows only when loaded are not;
- names: `field_names`;
- `nil`: omitted when `include_nil_values?` (the resource's, else the
  domain's) is false.

As with `serde_attributes`, a `field` setting that AshJsonApi's
configuration already decides (`rename`, `skip` of a listed field) is a
compile error; the others (`quantize`, `encode_as: :set`, …) mix in.
Adapters for other encoders (ash_jason, AshGraphql) can follow the same
pattern if wanted. OpenAPI is never a source: it describes AshJsonApi's
output, and a resource's identity is built from the resource
itself, as `construct_identity()` is in Rust.

`Quiddity.digest(record)` digests a record under its primary identity;
`Quiddity.digest(record, :meaning)` under another. `Quiddity.Ash.identity/2`
exports a resource's identity.

## E7. Ash types (Q6)

Proposed mapping: a type with a data-model kind is digested as that kind;
otherwise as its JSON-safe form (`dump_to_embedded/3`), as JSON encoders
write it. For the types below this is also what AshJsonApi exports.

| Ash type | Kind | Note |
|---|---|---|
| `:string` | string | |
| `:ci_string` | string, as written | The value keeps its case; only comparison ignores it. |
| `:integer`, `:float`, `:boolean` | integer, number, bool | |
| `:atom`, `Ash.Type.Enum` | string of the atom's name | Its dumped form. |
| `:uuid`, `:uuid_v7` | uuid | The canonical lowercase hyphenated text. |
| `:utc_datetime`, `:utc_datetime_usec`, `:datetime` | instant | Integer nanoseconds (spec 2.6). Written as RFC 3339 text, which an identity's `instant` kind reads to the same value. |
| `:naive_datetime` | string, ISO 8601 | No time zone, so not an instant (spec 2.6). |
| `:date`, `:time` | string, ISO 8601 | The data model has no date or time-of-day kind. |
| `:decimal` | string, its decimal text | Exact decimals are not in the data model (R4). `1.0` and `1.00` are then different strings (Q11). |
| `:duration` | string, ISO 8601 | Its dumped form. Months and years have no fixed length in nanoseconds, so not a duration kind. |
| `:duration_name`, `:module` | string | |
| `:binary` | bytes | |
| `{:array, t}` | list of `t` | `encode_as: :set` makes it a set. |
| `:map` with `fields` | object of those fields | |
| `:map` without `fields`, `:keyword` | map, as it is (`any`) | |
| `:struct` of an embedded resource | that resource's object, under its own `quiddity` configuration | Like a nested derived type. |
| `:struct` with `fields`, `:tuple` | object of those fields | |
| `:union` | by its `storage` | `:map_with_tag`: internally tagged on its `tag`; `:type_and_value` (the default): adjacently tagged, `type` and `value`; members without tags: an untagged union. |
| `Ash.Type.NewType` | its underlying type | With the new type's constraints. |
| `:vector` | list of numbers | |
| `:term`, `:function`, `:file`, `:range` | error, unless configured | No stable JSON-safe form. |
| other custom types | through `dump_to_embedded/3`, then the plain-value mapping (E3) | An error if the dumped value has no mapping. |

## E8. Dynamic identities (Q10)

The same API as the Rust crate's, in Elixir's form: an identity is
created in code or read from JSON, written out, and applied to
any value.

- **The type:** `%Quiddity.Identity{}`, always valid and canonical (spec
  5.3, 5.9), with `Quiddity.digest(value, identity)` digesting any value
  under it. Values are read as their JSON forms would be (E3), so a
  struct, a map decoded from JSON and an Ash record (E6) digest alike.
- **Kinds are plain terms**, the shapes of an identity's JSON form as
  atoms and tuples, so an identity reads like its JSON form: `:string`, `:integer`,
  `{:list, :string}`, `{:set, :string}`, `{:map, :string, :any}`,
  `{:optional, :string}`, `{:instant, unit: :ms}`, `{:object, fields}`
  with `fields` a keyword list of kinds or `{kind, opts}`, `{:tagged,
  variants, style: :internal, tag: "type"}`, `{:union, kinds}`,
  `{:ref, name}`. Elixir code builds data rather than objects; terms
  pattern-match, and need no builder modules.
- **Constructors and changes,** named as in Rust: `Identity.new/1`
  (`{:ok, identity}` or `{:error, %Quiddity.InvalidIdentity{path,
  message}}`) and `new!/1`; `named/2`, `set_domain/2`, `set_field/3`,
  `remove_field/2`, `set_field_in/4`, `remove_field_in/3`, `set_root/2`,
  `definition/2`. A nested `%Identity{}` inside a kind becomes a
  definition under its name, and `:recursive` refers to the identity
  being built, as in Rust.
- **JSON:** `Identity.from_json/1` takes JSON text or a decoded map,
  rejecting repeated keys (the built-in decoder keeps the last of repeated
  keys) and integers written as floats (P6); `Identity.to_json/1` gives
  the canonical JSON form as a map, and `JSON.Encoder` writes it.
- **Its own digest:** `Quiddity.digest(identity)`, under the identity of
  identities (Appendix C).
- **From types:** `Quiddity.identity(module)` exports the identity of a
  derived struct (E5) or, with `ash_quiddity`, of a resource (E6), as
  Rust's `construct_identity()` does.

## E9. Multiple identities

Elixir uses the Rust crate's model unchanged, so the README explains
identities once for every language:

1. The primary identity always exists. In Elixir it is named `:primary`,
   as `Primary` in Rust.
2. Additional identities are declared by name: an atom in Elixir, which
   needs no marker types.
3. Settings written without an identity apply to every identity; settings
   written for one identity replace them for that identity only (Rust's
   `skip` and `skip(Meaning)`).
4. The serde-style mode is scoped the same way: `follow :ash_json_api` at
   the top level applies to every identity, inside an `identity` block to
   that one (as `serde_attributes(Primary)`); D21's rule carries over:
   settings the followed encoder already decides are errors for the
   identities in the mode, and allowed for the others.
5. Each identity is exported on its own:
   `Quiddity.identity(Ticket, :meaning)`, as
   `<T as Quiddity<Meaning>>::construct_identity()`.

```elixir
# A plain struct
@derive {Quiddity.Encoder,
         except: [:submitted_at],                        # every identity
         fields: [asked_at: [quantize: 1_000_000_000]],  # every identity
         identities: [
           meaning: [fields: [priority: [skip: true]]]   # :meaning only
         ]}

# An Ash resource
quiddity do
  follow :ash_json_api                # every identity
  field :asked_at, quantize: 1_000_000_000

  identity :meaning do                # :meaning only
    field :priority, skip: true
  end
end

Quiddity.digest(ticket)               # the primary identity
Quiddity.digest(ticket, :meaning)
```

**Resolution:** an identity's settings for a field are its own block's
settings for that field merged over the unscoped ones, key by key, as
Rust resolves each attribute. An identity block's `only` or `except`, if
present, replaces the unscoped field selection: Rust selects per field
(`skip(Meaning)`), and `only` and `except` are Elixir's form of that
selection, so they are replaced whole.

**Why not Rust-only.** Ash is where named identities are most useful: a
resource is the type definition, and one record needs different digests
for different purposes (content for an ETag, meaning for deduplication,
routing), the README's own motivating cases. The model adds no concepts
beyond Rust's. The dynamic API (E8) already applies any number of
identities to any value, so named identities are a convenience for types,
as in Rust.

**Order of work.** Named identities are additive: they do not change the
primary identity. They follow the primary identity's implementation in
the core, the derive and the extension, and can be left out without
changing anything else.

## E10. Idiomatic Elixir

From Elixir's library guidelines, its design and macro anti-pattern
guides, its naming conventions and the `Protocol` documentation (read
2026-10-15), applied here:

- **Errors are values, with raising variants.** A function that can fail
  returns `{:ok, value}` or `{:error, reason}`, and its `!` variant raises,
  as `JSON.encode/1` and `JSON.encode!/1` do. So `Quiddity.digest/2`
  returns `{:ok, %Quiddity.Digest{}}` or `{:error, %Quiddity.Error{}}`,
  and `digest!/2` raises; `Identity.new/1` and `new!/1`, `from_json/1`
  and `from_json!/1` likewise. The error structs are exceptions
  (`defexception`), carrying `path` and `message` as `ApplyError` and
  `InvalidIdentity` do in Rust, so one struct serves both forms. Callers
  match on results rather than rescue (the "exceptions for control flow"
  anti-pattern).
- **Options are keyword lists, passed as arguments.** No application
  configuration: it is global, so two libraries could not configure
  quiddity differently (the "application configuration for libraries"
  anti-pattern). Options are validated, and an unknown option is an
  error.
- **Extension by protocol, not by `use`.** Types join quiddity by
  implementing the `Quiddity.Encoder` protocol, through `@derive` or by
  hand; `@derive` with options calls the protocol's `__deriving__/2`
  macro (Elixir 1.18). No `__using__/1`: `use` injects code a reader
  cannot see (the "`use` instead of `import`" anti-pattern). The
  generated implementation is small, holding the options and calling a
  shared function (the "large code generation" anti-pattern). The
  protocol does not fall back to `Any`: a struct that does not implement
  it is an error, as Elixir recommends for most protocols, and as `JSON`
  refuses such structs.
- **Names:** `!` for raising variants; `?` for booleans; `fetch` and
  `get` only for key-value lookups. Modules under `Quiddity`:
  `Quiddity.Digest`, `Quiddity.Identity`, `Quiddity.Encoder`,
  `Quiddity.Error`; the extension's under `AshQuiddity`.
- **Digests are a struct, not a string** (the "primitive obsession"
  anti-pattern): `%Quiddity.Digest{}` implements `String.Chars` (the text
  form), `Inspect` (`#Quiddity.Digest<sha256:…>`) and `JSON.Encoder`;
  `Quiddity.Digest.parse/1` reads the text form.
- **Publishing:** formatted code, ExUnit tests with doctests, `@doc` and
  `@spec` on public functions, ExDoc; version requirements as `"~> x.y"`;
  CI against the locked versions and against unlocked ones; the core
  built and tested without optional dependencies.

## Open questions

Resolved 2026-10-15:

- **Q1:** target Ash core, with an opt-in adapter for AshJsonApi
  (`follow :ash_json_api`); OpenAPI is not a source (E6).
- **Q2:** attributes only by default (E6).
- **Q3:** no policy from names; everything configurable per field (E6).
  No set of defaults is right for every type, so every default in this
  document can be changed per field or per type.
- **Q4:** what is not exported is not digested; in core's terms, what is
  not public (E6).
- **Q13:** the primary key has no special case. Whether an id is
  incidental or the crux of a record's meaning (a social security number)
  is the user's call, and how its value was produced (`generated?`, a
  default function) says nothing about it. A public key is digested like
  any public attribute and skipped with `field :id, skip: true`. In Ash
  3.33 the key shorthands (`uuid_primary_key`, `uuid_v7_primary_key`,
  `integer_primary_key`) default to `public? true`, and the timestamp
  shorthands to `public? false`. Under `follow :ash_json_api` the key is
  exported as the object's `id`, so it is digested too, under its
  attribute's name.

Recommended 2026-10-15, for review:

- **Q5:** keep "identity", in the spec and in every implementation. It
  says what an identity decides: when two values count as the same. It is
  in the spec (section 5, Appendix C), the vectors,
  the README and the Rust API. "Meaning" fits the primary identity but not
  every identity (a routing or cache identity is not a meaning); "view",
  "facet" and "aspect" suggest a partial look rather than a definition of
  sameness. In Elixir, identities are named by atoms (no marker modules,
  which Rust needs for its type system); the primary one is the top level
  of a derive or a `quiddity` section. Additional ones are declared with
  `identities:` in a derive, as Rust's `identities(Meaning)`, and with
  `identity :name do … end` inside the `quiddity` section in Ash. Ash's
  own `identity` entity (a unique key) lives in its `identities` section;
  Spark scopes entities to their sections, so the two do not collide, and
  the section says which is meant. `digest` was considered for the
  keyword and rejected: `Quiddity.digest/2` is the function and
  `%Quiddity.Digest{}` its result, so it would name three things.

- **Q6:** the Ash type mapping in E7.
- **Q7:** the derive's options in E5: `@derive` with a keyword list whose
  options are the Ash section's.
- **Q8:** two packages, `quiddity` and `ash_quiddity` (E2).
- **Q9:** plain values follow Elixir's JSON encoders, except where the
  data model is richer than JSON (E3).
- **Q10:** the dynamic identity API in E8.
- **Q11:** a `Decimal` digests as its text, as both JSON encoders write
  it, so `1.0` and `1.00` differ, as they do on the wire. Per field,
  `decimals: d` quantizes it (exactly, as for floats), which also makes
  `1.0` and `1.00` equal; an exact decimal kind in the data model (R4)
  remains the lasting answer, and needs the other implementations.
- **Q12:** under `follow :ash_json_api`, calculations and aggregates
  listed in `default_fields` are digested, and one that is not loaded is
  an error (E6). Without the adapter, they are digested only when listed
  in the `quiddity` section. In ULP this covers a predicate's localized
  text, which is a calculation listed in `default_fields`.
- **Q14:** public `sensitive?` attributes are left out by core's default,
  as Ash and ash_jason treat them, and digested under `follow
  :ash_json_api`, which exports them in the clear (E6). Either way, a
  `field` setting changes it.

Remaining unknowns, found 2026-10-15:

- **Q15. Records that are not fully loaded.** An Ash record can hold
  `%Ash.NotLoaded{}` for an attribute that a query did not select, and
  `%Ash.ForbiddenField{}` for one a field policy hides from the actor.
  AshJsonApi and ash_jason omit both. A digest must not depend on the
  query or the actor, so either is an error naming the field. To be
  confirmed against field policies in ULP.
- **Q16. A digest as a field of the resource.** The idiomatic way to give
  a record a derived value in Ash is a calculation. `ash_quiddity` could
  provide one, `calculate :etag, :string, {AshQuiddity.Calculation,
  identity: :primary}`, so a digest can be loaded with `Ash.load/2`,
  exported by AshJsonApi, and used where the record is used. An Ash type
  for storing digests (`AshQuiddity.Type.Digest`) and a change that
  stores one on write would follow. Proposed for after the extension
  itself.
- **Q17. Compile-time dependencies between resources.** An embedded
  resource's digest uses its own `quiddity` configuration, and a listed
  relationship refers to another resource. Reading another resource's
  configuration while compiling makes resources depend on each other at
  compile time. Spark's verifiers run after compilation and are, as far
  as can be told so far, the place for such checks; the digesting itself
  reads the configuration at run time. To be confirmed against Spark's
  documentation and source.
- **Q18. Jason.** Most applications that use Ash, ULP among them, encode
  with Jason. `%Quiddity.Digest{}` and `%Quiddity.Identity{}` implement
  `JSON.Encoder`; implementing `Jason.Encoder` too, when Jason is present
  (an optional dependency), lets them be encoded with either.
- **Q19. Float text.** Quantizing a float to decimals takes it as its
  shortest round-trip decimal (spec 5.6, D20). OTP 25 and later give that
  with `:erlang.float_to_binary(x, [:short])` (on OTP 28: `0.1` is
  `"0.1"`, `2.67` is `"2.67"`, `1.0e300` is `"1.0e300"`); the vectors
  will confirm it.


## Provisional decisions (implementation, 2026-10-15)

Taken while implementing `elixir/quiddity`, where the choice was not clear,
to be revisited. Each says what was chosen, the alternatives, and why.

- **P1. A data-model value type.** `Quiddity.Value` holds values of the data
  model: Elixir scalars, binaries, and tagged tuples for lists, sets, maps,
  instants, durations and the floats the BEAM cannot represent (NaN, ±∞).
  The encoder works only on it; `Quiddity.Value.from/1` reads Elixir terms
  into it; `Quiddity.Encoder` implementations return it. Alternative:
  encode Elixir terms directly. Chosen because the vectors need values
  Elixir cannot hold (NaN, text with lone surrogates), and because an
  identity must tell an instant from an integer, as Rust's source tree does.
- **P2. Binaries are text under an identity.** Elixir does not tell bytes
  from text, so a binary is a string source; a `bytes` kind reads it as
  base64 (or hex) text, as from JSON. Raw bytes are given as a list of
  integers. Alternative: read binaries that are not valid UTF-8 as bytes.
  Rejected as guessing.
- **P3. A struct under an identity is read as its `Quiddity.Encoder` form**,
  not its `JSON.Encoder` form: values are read through `Quiddity.Value.from/1`
  before the identity applies. For derived structs with matching options the
  two agree (a test checks the exported identity against `JSON.encode!`).
  Alternative: require `JSON.Encoder`. Rejected: many structs have only one.
- **P4. Plain values follow Elixir's JSON encoders (E3), checked against
  Elixir 1.19's `JSON` and Jason 1.4**, with one exception decided here:
- **P5. `Decimal` as its scientific string.** `JSON` writes `1E+10`, Jason
  `10000000000` (`Decimal.to_string(d, :normal)`). First chosen: Jason's,
  since ULP and most Ash applications encode with Jason. Changed
  2026-10-15: `Decimal.to_string/1`'s default, the General Decimal
  Arithmetic specification's "to-scientific-string", which Elixir's `JSON`
  writes and which Python's `str(Decimal)` implements too (checked on 13
  values, among them `1E+10`, `1.2E-7`, `0E-7`, `-0` and `12.50`), so that
  a decimal digests alike in Elixir and Python (PYTHON.md, Q5). `Decimal`
  is matched by its struct name, so it is not a dependency.
- **P6. An identity's integers are integers** (resolved 2026-10-15,
  DESIGN.md D22). `quantize`, `decimals` and `discriminant` written as
  `5.0` are rejected, as the Rust crate rejects them. This package first
  read an identity's numbers by spec 2.7, so that `5.0` was `5`, but 2.7
  is about values being digested, not an identity's own members.
  Identities have no version member either (D22).
- **P7. One `Quiddity.Digest` struct for 256, 128 and 64 bits**, told apart
  by size, where Rust has three types; `mini/1` and `micro/1` return it.
  Fewer types, as in the rest of the Elixir API.
- **P8. A derived struct's configuration is in its own module**, as a
  hidden `__quiddity__/1`: protocol implementation modules keep only the
  protocol's functions. `Quiddity.digest/2` reads the domain from it, and
  `Quiddity.identity/2` the configuration. A hand-written implementation
  has no domain (use `Quiddity.Digest.in_domain/2`) unless its struct's
  module defines `__quiddity__({:domain, identity})`. To revisit: a second
  protocol function, or a documented callback.
- **P9. `__deriving__/2` is defined in the protocol**, as `JSON.Encoder` does
  since Elixir 1.18. Defined in an `Any` implementation, Elixir takes its
  legacy path and silently derives a delegation to `Any`.
- **P10. Derive option names.** `omit_nil` for Rust's `omit_none` (Elixir's
  word for null); in identities built in code, `field(kind, optional: true)`
  for Rust's `Field::optional`. Document members keep the spec's names
  (`"omit": ["none"]`).
- **P11. Exported field kinds.** A derived field exports as the kind its
  `kind:` option declares; else as the kind the struct's `@type t`
  typespec gives it (added 2026-10-15; `Quiddity.Typespec` maps the usual
  types, and another struct's `t()` to its identity, nested, a struct's
  own `t()` to a recursive kind, and longer cycles to `:any`); else as
  `:any`. Typespecs are read from the compiled module's debug information,
  which a default `mix release` strips; exporting then never fails, but
  gives `:any`. A type it does not recognize is `:any`, and so is a union
  that would not be a valid kind. A flattened field cannot be exported,
  and is an error. Elixir structs declare no field types besides
  typespecs (Elixir 1.20 infers types but exposes none to libraries).
- **P12. `flatten` in the derive** merges the field's data-model map into
  the struct's; `nil` adds nothing; a repeated key is a `Quiddity.Error`
  (not a crash, since digesting returns errors in Elixir).
- **P13. `Quiddity.digest/2`'s second argument** is an identity's name (an
  atom) or a `%Quiddity.Identity{}`; there are no keyword options. Domains
  come from the derive, from identities, or from `Digest.in_domain/2`.
- **P14. Canonical order of union alternatives: by their encodings**, as in
  Rust (`qident.py` orders by JSON text). The order never reaches a digest
  (unions are sets in the meta-identity), only equality of identities.
- **P15. Number text beyond binary64** (`"1e400"`) under a `number` kind is
  ±∞, the nearest binary64 value; integer text with more than 4096 digits
  is an error, as in Rust.
- **P16. Protocol consolidation is off in the test environment**, as the
  `Protocol` documentation suggests, since tests define structs that derive
  `Quiddity.Encoder`.

Checked: the language-neutral vectors (all groups), and digests computed
by the Rust crate (`examples/dynamic_identity.rs`: a value's digest, the
digest after removing a field, and the identity's own digest) agree.

## Provisional decisions (ash_quiddity, 2026-10-15)

- **A1. Fields are resolved at run time, and cached.** Which fields an
  identity covers depends on the resource, its `quiddity` section and, when
  followed, its `json_api` section. They are resolved on first use and
  cached in `:persistent_term`, keyed by the resource's module checksum so
  that recompiling it in development replaces the entry. Alternative:
  resolve them in a transformer at compile time. Rejected for now: it
  would read other extensions' sections (AshJsonApi) and other resources
  (embedded ones, relationships) while compiling, the compile-time
  dependencies Q17 is about.
- **A2. The extension adds code with `Spark.Dsl.Transformer.eval/3`**: the
  `Quiddity.Encoder` implementation for the resource's records, and the
  hidden `__quiddity__/1` hook (P8) through which `Quiddity.digest/2` reads
  the domain and `Quiddity.identity/2` exports. `Quiddity.identity/2` asks
  the module through this hook, so derived structs and Ash resources
  export alike.
- **A3. A verifier checks the section** (fields exist, identity names, no
  `rename` when following AshJsonApi, not `quantize` and `decimals`
  together). Verifiers run after compilation, as Q17 suggested; their
  errors fail `mix compile`. Tests collect them with `Spark.Test`.
- **A4. Values are read through `Ash.Type.dump_to_embedded/3`**, then as
  Quiddity reads Elixir values (E7); if a type cannot dump a value, the
  value itself is read. A record of an embedded resource is digested under
  its own section (or the defaults, if it has none), and related records
  under their resource's.
- **A5. Sensitive attributes:** left out under core's rules, digested when
  following AshJsonApi, which exports them (Q14). The primary key is
  digested when following AshJsonApi (Q13), and under core's rules when it
  is public, which Ash's key shorthands are.
- **A6. Exported kinds for types without a data-model kind:** `:binary` is
  exported as `:string` (Ash writes binaries as text in JSON), unions and
  types with no mapping as `:any`, aggregates whose type Ash leaves unset
  as `:any`. A nullable attribute is `{:optional, kind}`. To revisit with
  real resources (ULP).
- **A7. Nested records have no domain**: only the top-level record's
  domain applies, as in Rust.
- **A8. The calculation's `load/3`** asks for every field the identity
  covers, relationships as `{name, []}`; `form:` chooses the 256-, 128- or
  64-bit text form.
- **A9. Not tried against ULP.** The extension is tested with its own test
  resources (ETS and embedded data layers, AshJsonApi with `field_names`,
  `hide_fields` and `include_nil_values?`). ULP's resources (fragments,
  field policies, translations as calculations) are the next test.

