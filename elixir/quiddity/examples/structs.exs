# Digests of structs, with @derive Quiddity.Encoder.
#
#     mix run examples/structs.exs

defmodule MyApp.Author do
  @derive [Quiddity.Encoder, JSON.Encoder]
  defstruct [:name, :born]
end

defmodule MyApp.Book do
  @derive {Quiddity.Encoder,
           except: [:last_opened],
           domain: "example.book",
           fields: [
             genres: [encode_as: :set],
             catalogued_at: [quantize: 1_000_000_000, kind: :instant],
             price: [decimals: 2, kind: :number],
             subtitle: [omit_nil: true, kind: :string]
           ],
           identities: [
             meaning: [fields: [pages: [skip: true]]]
           ]}
  @derive JSON.Encoder
  defstruct [:title, :authors, :pages, :genres, :catalogued_at, :price, :subtitle, :last_opened]
end

# A script is compiled as a whole, so it builds the structs it defines with
# struct!/2 rather than %MyApp.Book{} literals.
book =
  struct!(MyApp.Book,
    title: "Moby Dick",
    authors: [struct!(MyApp.Author, name: "Herman Melville", born: 1819)],
    pages: 635,
    genres: ["novel", "adventure"],
    catalogued_at: ~U[2026-10-15 09:30:00.250Z],
    price: 12.5,
    subtitle: nil,
    # When you last read it: not part of the book.
    last_opened: DateTime.utc_now()
  )

digest = Quiddity.digest!(book)
IO.puts("primary:   #{digest}")
IO.puts("meaning:   #{Quiddity.digest!(book, :meaning)}")

# Fields that do not count, or that differ by less than the grid, or in
# order only, leave the digest unchanged.
same = %{
  book
  | last_opened: ~U[2000-01-01 00:00:00Z],
    genres: ["adventure", "novel"],
    price: 12.501
}

IO.puts("unchanged: #{Quiddity.digest!(same) == digest}")

# Another edition, with another page count, changes the primary digest,
# not :meaning's: the work, whatever the edition.
abridged = %{book | pages: 320}
IO.puts("primary changed: #{Quiddity.digest!(abridged) != digest}")

IO.puts(
  "meaning same:    #{Quiddity.digest!(abridged, :meaning) == Quiddity.digest!(book, :meaning)}"
)

# The struct's identity as data, for another language to apply to its JSON
# form. Kinds come from `kind:` options, or from a @type t typespec in a
# compiled module (a script's modules have none), or are :any.
{:ok, identity} = Quiddity.identity(MyApp.Book)
IO.puts("\nidentity as JSON:\n#{JSON.encode!(identity)}\n")

json = book |> JSON.encode!() |> JSON.decode!()
IO.puts("JSON form digests alike: #{Quiddity.digest!(json, identity) == digest}")
