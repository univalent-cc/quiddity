# Digests of Ash records, with the AshQuiddity.Resource extension.
#
#     mix run examples/helpdesk.exs

# Ash logs each record it creates; this script prints its own results.
Logger.configure(level: :info)
require Ash.Query

defmodule Helpdesk.Address do
  use Ash.Resource, data_layer: :embedded, extensions: [AshQuiddity.Resource]

  attributes do
    attribute(:city, :string, public?: true)
    attribute(:zip, :string, public?: true)
  end
end

defmodule Helpdesk.Ticket do
  use Ash.Resource,
    domain: Helpdesk,
    data_layer: Ash.DataLayer.Ets,
    extensions: [AshQuiddity.Resource]

  attributes do
    uuid_primary_key(:id)
    attribute(:subject, :string, public?: true, allow_nil?: false)
    attribute(:priority, :integer, public?: true)
    attribute(:tags, {:array, :string}, public?: true, default: [])
    attribute(:asked_at, :utc_datetime_usec, public?: true)
    attribute(:address, Helpdesk.Address, public?: true)
    # Not public: not digested.
    attribute(:internal_note, :string)
    timestamps()
  end

  calculations do
    calculate(:etag, :string, {AshQuiddity.Calculation, form: :micro})
  end

  actions do
    default_accept([:subject, :priority, :tags, :asked_at, :address, :internal_note])
    defaults([:read, :create, :update])
  end

  quiddity do
    domain("helpdesk.ticket")
    # A generated id is incidental here: two tickets that say the same
    # thing have the same digest.
    field(:id, skip: true)
    field(:tags, encode_as: :set)
    field(:asked_at, quantize: 1_000_000_000)

    identity :meaning do
      field(:priority, skip: true)
    end
  end
end

defmodule Helpdesk do
  use Ash.Domain

  resources do
    resource(Helpdesk.Ticket)
  end
end

create = fn attrs ->
  Helpdesk.Ticket
  |> Ash.Changeset.for_create(:create, attrs)
  |> Ash.create!()
end

ticket =
  create.(%{
    subject: "Printer jam",
    priority: 1,
    tags: ["hardware", "office"],
    asked_at: ~U[2026-10-15 09:30:00.250000Z],
    address: %{city: "Lyon", zip: "69001"},
    internal_note: "call first"
  })

digest = Quiddity.digest!(ticket)
IO.puts("digest:          #{digest}")

# Another ticket that says the same thing: another id, the tags in another
# order, another note, the same second.
same =
  create.(%{
    subject: "Printer jam",
    priority: 1,
    tags: ["office", "hardware"],
    asked_at: ~U[2026-10-15 09:30:00.900000Z],
    address: %{city: "Lyon", zip: "69001"},
    internal_note: "ask for Sam"
  })

IO.puts("same meaning:    #{Quiddity.digest!(same) == digest}")

# :meaning leaves the priority out.
urgent =
  create.(%{
    subject: "Printer jam",
    priority: 9,
    tags: ["office", "hardware"],
    address: %{city: "Lyon", zip: "69001"},
    asked_at: ~U[2026-10-15 09:30:00Z]
  })

IO.puts("primary differs: #{Quiddity.digest!(urgent) != digest}")

IO.puts(
  "meaning same:    #{Quiddity.digest!(urgent, :meaning) == Quiddity.digest!(ticket, :meaning)}"
)

# A record read with only some of its fields cannot be digested: the digest
# would be of another value.
[partial | _] = Helpdesk.Ticket |> Ash.Query.select([:subject]) |> Ash.read!()
{:error, error} = Quiddity.digest(partial)
IO.puts("partial record:  #{Exception.message(error)}")

# The digest as a calculation: computed only when loaded, and Ash loads the
# fields it covers first, even when the query selects fewer.
[loaded] =
  Helpdesk.Ticket
  |> Ash.Query.filter(id == ^ticket.id)
  |> Ash.Query.select([:subject])
  |> Ash.Query.load(:etag)
  |> Ash.read!()

IO.puts("etag:            #{loaded.etag}")
IO.puts("is the digest's:  #{loaded.etag == to_string(Quiddity.Digest.micro(digest))}")

# The resource's identity as data, for other languages to apply to a
# record's JSON form.
{:ok, identity} = Quiddity.identity(Helpdesk.Ticket)
IO.puts("\nidentity document:\n#{JSON.encode!(identity)}")
