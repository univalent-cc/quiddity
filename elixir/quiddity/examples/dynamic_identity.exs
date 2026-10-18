# Identities built at run time, applied to values decoded from JSON.
#
# The same identity as the Rust crate's examples/dynamic_identity.rs, which
# prints the same digests.
#
#     mix run examples/dynamic_identity.exs

alias Quiddity.Identity
import Quiddity.Identity, only: [field: 2]

# --- Building ------------------------------------------------------------------

# An identity with a name becomes a definition of the identities it is
# nested in, under that name.
author =
  {:object, name: :string, born: :integer}
  |> Identity.new!()
  |> Identity.named("Author")

# Each variant of a tagged union is a unit variant (nil) or an object.
format =
  {:tagged, [Ebook: nil, Print: {:object, volumes: :integer}], style: :internal, tag: "type"}

book =
  {:object,
   [
     title: :string,
     authors: {:list, author},
     pages: :integer,
     format: format,
     # Left out when null or missing.
     subtitle: field(:string, optional: true),
     # Left out when empty; the order of its members does not count.
     genres: field({:set, :string}, omit_empty: true),
     # When the library record was made. Read from the member cataloguedAt,
     # digested under the key catalogued_at, and rounded down to the second.
     cataloguedAt: field(:instant, rename: "catalogued_at", quantize: 1_000_000_000),
     # The book this one follows, or null: :recursive is the identity being
     # built.
     follows: field(:recursive, optional: true)
   ]}
  |> Identity.new!(domain: "example.book")
  |> Identity.named("Book")

# --- Digesting -----------------------------------------------------------------

value = %{
  "title" => "Moby Dick",
  "authors" => [%{"name" => "Herman Melville", "born" => 1819}],
  "pages" => 635,
  "format" => %{"type" => "Print", "volumes" => 3},
  "genres" => ["novel", "adventure"],
  "cataloguedAt" => "2026-10-14T09:30:00.250Z",
  # Not a field: skipped.
  "last_opened" => "2026-10-16T21:05:00Z"
}

digest = Quiddity.digest!(value, book)
IO.puts("digest:            #{digest}")

# The same book, written differently: members in another order, the integer
# as text, the instant in another time zone, the set reordered.
same = %{
  "cataloguedAt" => "2026-10-14T11:30:00+02:00",
  "genres" => ["adventure", "novel"],
  "subtitle" => nil,
  "format" => %{"volumes" => 3, "type" => "Print"},
  "pages" => "635",
  "authors" => [%{"born" => 1819, "name" => "Herman Melville"}],
  "title" => "Moby Dick"
}

IO.puts("written differently: same digest: #{Quiddity.digest!(same, book) == digest}")

# A recursive value.
typee = %{value | "title" => "Typee", "pages" => 298}
IO.puts("follows Typee:     #{Quiddity.digest!(Map.put(value, "follows", typee), book)}")

# A value of the wrong shape is an error that says where.
wrong = put_in(value, ["authors"], [%{"name" => 7, "born" => 1819}])
{:error, error} = Quiddity.digest(wrong, book)
IO.puts("wrong shape:       #{Exception.message(error)}")

# --- Changing ------------------------------------------------------------------

# A change that would make the identity invalid fails and changes nothing.
{:error, error} = Identity.set_field_in(book, "Book", "name", field(:string, rename: "title"))
IO.puts("invalid change:    #{Exception.message(error)}")

{:ok, book} = Identity.remove_field_in(book, "Book", "pages")
IO.puts("without pages:     #{Quiddity.digest!(value, book)}")

# --- As JSON -------------------------------------------------------------------

# An identity is data: written out as JSON, any implementation reads it.
json = Identity.to_json(book)
{:ok, loaded} = Identity.from_json(JSON.encode!(json))
IO.puts("loaded back:       #{loaded == book}")

# An identity has a digest of its own, the same in every implementation.
IO.puts("identity's digest: #{Quiddity.digest!(book)}")
