# Digests of structs, with @derive Quiddity.Encoder.
#
#     mix run examples/structs.exs

defmodule MyApp.DocumentRef do
  @derive [Quiddity.Encoder, JSON.Encoder]
  defstruct [:label, :content]
end

defmodule MyApp.Task do
  @derive {Quiddity.Encoder,
           except: [:submitted_at],
           domain: "example.task",
           fields: [
             tags: [encode_as: :set],
             asked_at: [quantize: 1_000_000_000, kind: :instant],
             budget: [decimals: 2, kind: :number],
             note: [omit_nil: true, kind: :string]
           ],
           identities: [
             meaning: [fields: [priority: [skip: true]]]
           ]}
  @derive JSON.Encoder
  defstruct [:prompt, :documents, :priority, :tags, :asked_at, :budget, :note, :submitted_at]
end

# A script is compiled as a whole, so it builds the structs it defines with
# struct!/2 rather than %MyApp.Task{} literals.
task =
  struct!(MyApp.Task,
    prompt: "Summarize the report",
    documents: [
      struct!(MyApp.DocumentRef, label: "report", content: Quiddity.digest!("The report's text"))
    ],
    priority: 1,
    tags: ["finance", "q3"],
    asked_at: ~U[2026-10-15 09:30:00.250Z],
    budget: 12.5,
    note: nil,
    submitted_at: DateTime.utc_now()
  )

digest = Quiddity.digest!(task)
IO.puts("primary:   #{digest}")
IO.puts("meaning:   #{Quiddity.digest!(task, :meaning)}")

# Fields that do not count, or that differ by less than the grid, or in
# order only, leave the digest unchanged.
same = %{task | submitted_at: ~U[2000-01-01 00:00:00Z], tags: ["q3", "finance"], budget: 12.501}
IO.puts("unchanged: #{Quiddity.digest!(same) == digest}")

# A change in priority changes the primary digest, not :meaning's.
urgent = %{task | priority: 9}
IO.puts("primary changed: #{Quiddity.digest!(urgent) != digest}")

IO.puts(
  "meaning same:    #{Quiddity.digest!(urgent, :meaning) == Quiddity.digest!(task, :meaning)}"
)

# The struct's identity as data, for another language to apply to its JSON
# form. Kinds come from `kind:` options, or from a @type t typespec in a
# compiled module (a script's modules have none), or are :any.
{:ok, identity} = Quiddity.identity(MyApp.Task)
IO.puts("\nidentity document:\n#{JSON.encode!(identity)}\n")

json = task |> JSON.encode!() |> JSON.decode!()
IO.puts("JSON form digests alike: #{Quiddity.digest!(json, identity) == digest}")
