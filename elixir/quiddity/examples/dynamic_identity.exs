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
document_ref =
  {:object, label: :string, content: :string}
  |> Identity.new!()
  |> Identity.named("DocumentRef")

# Each variant of a tagged union is a unit variant (nil) or an object.
answer_format =
  {:tagged, [Free: nil, Paragraphs: {:object, count: :integer}], style: :internal, tag: "type"}

task =
  {
    :object,
    # Left out when null or missing.
    # Left out when empty; the order of its members does not count.
    # Read from the member askedAt, digested under the key asked_at, and
    # rounded down to the second.
    # Another task, or null: :recursive is the identity being built.
    prompt: :string,
    documents: {:list, document_ref},
    priority: :integer,
    format: answer_format,
    note: field(:string, optional: true),
    tags: field({:set, :string}, omit_empty: true),
    askedAt: field(:instant, rename: "asked_at", quantize: 1_000_000_000),
    follows: field(:recursive, optional: true)
  }
  |> Identity.new!(domain: "example.task")
  |> Identity.named("Task")

# --- Digesting -----------------------------------------------------------------

value = %{
  "prompt" => "Summarize the report",
  "documents" => [%{"label" => "report", "content" => "sha256:9f2c…"}],
  "priority" => 1,
  "format" => %{"type" => "Paragraphs", "count" => 2},
  "tags" => ["finance", "q3"],
  "askedAt" => "2026-10-14T09:30:00.250Z",
  # Not a field: skipped.
  "submitted_by" => "alice"
}

digest = Quiddity.digest!(value, task)
IO.puts("digest:            #{digest}")

# The same task, written differently: members in another order, the integer
# as text, the instant in another time zone, the set reordered.
same = %{
  "askedAt" => "2026-10-14T11:30:00+02:00",
  "tags" => ["q3", "finance"],
  "note" => nil,
  "format" => %{"count" => 2, "type" => "Paragraphs"},
  "priority" => "1",
  "documents" => [%{"content" => "sha256:9f2c…", "label" => "report"}],
  "prompt" => "Summarize the report"
}

IO.puts("written differently: same digest: #{Quiddity.digest!(same, task) == digest}")

# A value of the wrong shape is an error that says where.
wrong = put_in(value, ["documents"], [%{"label" => 7, "content" => "x"}])
{:error, error} = Quiddity.digest(wrong, task)
IO.puts("wrong shape:       #{Exception.message(error)}")

# --- Changing ------------------------------------------------------------------

# A change that would make the identity invalid fails and changes nothing.
{:error, error} = Identity.set_field_in(task, "Task", "title", field(:string, rename: "prompt"))
IO.puts("invalid change:    #{Exception.message(error)}")

{:ok, task} = Identity.remove_field_in(task, "Task", "priority")
IO.puts("without priority:  #{Quiddity.digest!(value, task)}")

# --- Documents -----------------------------------------------------------------

# An identity is data: written out as JSON, any implementation reads it.
document = Identity.to_json(task)
{:ok, loaded} = Identity.from_json(JSON.encode!(document))
IO.puts("loaded back:       #{loaded == task}")

# An identity has a digest of its own, the same in every implementation.
IO.puts("identity's digest: #{Quiddity.digest!(task)}")
