# Digests of Elixir values.
#
#     mix run examples/values.exs

# Two values that mean the same thing have the same digest: map keys in any
# order, atom or string keys, 1 and 1.0, a DateTime in any time zone.
a =
  Quiddity.digest!(%{
    title: "Moby Dick",
    pages: 635,
    published: ~U[1851-10-18 00:00:00Z]
  })

b =
  Quiddity.digest!(%{
    "published" => DateTime.shift_zone!(~U[1851-10-18 00:00:00Z], "Etc/UTC"),
    "pages" => 635.0,
    "title" => "Moby Dick"
  })

IO.puts("digest:         #{a}")
IO.puts("same value:     #{a == b}")

# A set's order does not count; a list's does.
IO.puts(
  "sets:           #{Quiddity.digest!(MapSet.new([1, 2])) == Quiddity.digest!(MapSet.new([2, 1]))}"
)

IO.puts("lists:          #{Quiddity.digest!([1, 2]) == Quiddity.digest!([2, 1])}")

# What JSON cannot write is an error, which says where.
{:error, error} = Quiddity.digest(%{steps: [1, {:pending, 2}]})
IO.puts("error:          #{Exception.message(error)}")

# Shorter forms, for an ETag for example, and the text form read back.
IO.puts("micro:          #{Quiddity.Digest.micro(a)}")
{:ok, parsed} = Quiddity.Digest.parse(to_string(a))
IO.puts("parsed back:    #{parsed == a}")

# A domain keeps digests of different kinds of values apart.
payment =
  Quiddity.digest!(%{to: "bob", amount: 100}) |> Quiddity.Digest.in_domain("example.payment")

refund =
  Quiddity.digest!(%{to: "bob", amount: 100}) |> Quiddity.Digest.in_domain("example.refund")

IO.puts("domains differ: #{payment != refund}")
