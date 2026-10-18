# Digests of Ash records, with the AshQuiddity.Resource extension.
#
#     mix run examples/library.exs

# Ash logs each record it creates; this script prints its own results.
Logger.configure(level: :info)
require Ash.Query

defmodule Library.Publisher do
  use Ash.Resource, data_layer: :embedded, extensions: [AshQuiddity.Resource]

  attributes do
    attribute(:name, :string, public?: true)
    attribute(:city, :string, public?: true)
  end
end

defmodule Library.Book do
  use Ash.Resource,
    domain: Library,
    data_layer: Ash.DataLayer.Ets,
    extensions: [AshQuiddity.Resource]

  attributes do
    uuid_primary_key(:id)
    attribute(:title, :string, public?: true, allow_nil?: false)
    attribute(:pages, :integer, public?: true)
    attribute(:genres, {:array, :string}, public?: true, default: [])
    # When the library record was made.
    attribute(:catalogued_at, :utc_datetime_usec, public?: true)
    attribute(:publisher, Library.Publisher, public?: true)
    # Not public: not digested.
    attribute(:shelf_note, :string)
    timestamps()
  end

  calculations do
    calculate(:etag, :string, {AshQuiddity.Calculation, form: :micro})
  end

  actions do
    default_accept([:title, :pages, :genres, :catalogued_at, :publisher, :shelf_note])
    defaults([:read, :create, :update])
  end

  quiddity do
    domain("library.book")
    # A generated id is incidental here: two copies of the same book have
    # the same digest.
    field(:id, skip: true)
    field(:genres, encode_as: :set)
    field(:catalogued_at, quantize: 1_000_000_000)

    # The work, whatever the edition.
    identity :meaning do
      field(:pages, skip: true)
    end
  end
end

defmodule Library do
  use Ash.Domain

  resources do
    resource(Library.Book)
  end
end

create = fn attrs ->
  Library.Book
  |> Ash.Changeset.for_create(:create, attrs)
  |> Ash.create!()
end

book =
  create.(%{
    title: "Moby Dick",
    pages: 635,
    genres: ["novel", "adventure"],
    catalogued_at: ~U[2026-10-15 09:30:00.250000Z],
    publisher: %{name: "Harper & Brothers", city: "New York"},
    shelf_note: "spine is loose"
  })

digest = Quiddity.digest!(book)
IO.puts("digest:          #{digest}")

# Another copy of the same book: another id, the genres in another order,
# another shelf note, the same second.
same =
  create.(%{
    title: "Moby Dick",
    pages: 635,
    genres: ["adventure", "novel"],
    catalogued_at: ~U[2026-10-15 09:30:00.900000Z],
    publisher: %{name: "Harper & Brothers", city: "New York"},
    shelf_note: "returned late"
  })

IO.puts("same book:       #{Quiddity.digest!(same) == digest}")

# :meaning leaves the page count out.
abridged =
  create.(%{
    title: "Moby Dick",
    pages: 320,
    genres: ["adventure", "novel"],
    publisher: %{name: "Harper & Brothers", city: "New York"},
    catalogued_at: ~U[2026-10-15 09:30:00Z]
  })

IO.puts("primary differs: #{Quiddity.digest!(abridged) != digest}")

IO.puts(
  "meaning same:    #{Quiddity.digest!(abridged, :meaning) == Quiddity.digest!(book, :meaning)}"
)

# A record read with only some of its fields cannot be digested: the digest
# would be of another value.
[partial | _] = Library.Book |> Ash.Query.select([:title]) |> Ash.read!()
{:error, error} = Quiddity.digest(partial)
IO.puts("partial record:  #{Exception.message(error)}")

# The digest as a calculation: computed only when loaded, and Ash loads the
# fields it covers first, even when the query selects fewer.
[loaded] =
  Library.Book
  |> Ash.Query.filter(id == ^book.id)
  |> Ash.Query.select([:title])
  |> Ash.Query.load(:etag)
  |> Ash.read!()

IO.puts("etag:            #{loaded.etag}")
IO.puts("is the digest's:  #{loaded.etag == to_string(Quiddity.Digest.micro(digest))}")

# The resource's identity as data, for other languages to apply to a
# record's JSON form.
{:ok, identity} = Quiddity.identity(Library.Book)
IO.puts("\nidentity as JSON:\n#{JSON.encode!(identity)}")
