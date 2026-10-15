defmodule Quiddity.DeriveTest do
  use ExUnit.Case, async: true

  alias Quiddity.{Digest, Identity}

  defmodule DocumentRef do
    @derive [Quiddity.Encoder, JSON.Encoder]
    defstruct [:label, :content]
  end

  defmodule Task do
    @derive {Quiddity.Encoder,
             except: [:submitted_at],
             domain: "example.task",
             fields: [
               note: [omit_nil: true],
               tags: [encode_as: :set, omit_empty: true],
               asked_at: [quantize: 1_000_000_000],
               budget: [decimals: 2],
               prompt: [rename: "question"]
             ],
             identities: [
               meaning: [fields: [priority: [skip: true]]]
             ]}
    defstruct [:prompt, :priority, :note, :tags, :asked_at, :budget, :documents, :submitted_at]
  end

  defp task(overrides \\ []) do
    struct!(
      %Task{
        prompt: "Summarize the report",
        priority: 1,
        note: nil,
        tags: ["finance", "q3"],
        asked_at: ~U[2026-10-14 09:30:00.250Z],
        budget: 12.5,
        documents: [%DocumentRef{label: "report", content: Quiddity.digest!("The report")}],
        submitted_at: DateTime.utc_now()
      },
      overrides
    )
  end

  test "fields that do not count leave the digest unchanged" do
    base = Quiddity.digest!(task())
    assert Quiddity.digest!(task(submitted_at: ~U[2000-01-01 00:00:00Z])) == base
    assert Quiddity.digest!(task(tags: ["q3", "finance"])) == base
    assert Quiddity.digest!(task(asked_at: ~U[2026-10-14 09:30:00.900Z])) == base
    assert Quiddity.digest!(task(budget: 12.501)) == base
    refute Quiddity.digest!(task(priority: 2)) == base
  end

  test "the digest is the map it describes, in the struct's domain" do
    t = task(tags: [], budget: 12.5)

    expected =
      Quiddity.digest!(%{
        "question" => t.prompt,
        "priority" => 1,
        "asked_at" => 1_791_970_200_000_000_000,
        "budget" => 12.5,
        "documents" => [
          %{"label" => "report", "content" => to_string(Quiddity.digest!("The report"))}
        ]
      })
      |> Digest.in_domain("example.task")

    # A nested digest is digested as its text form.
    assert Quiddity.digest!(%{d: Quiddity.digest!("x")}) ==
             Quiddity.digest!(%{d: to_string(Quiddity.digest!("x"))})

    assert Quiddity.digest!(t) == expected
  end

  test "additional identities" do
    a = task(priority: 1)
    b = task(priority: 9)
    refute Quiddity.digest!(a) == Quiddity.digest!(b)
    assert Quiddity.digest!(a, :meaning) == Quiddity.digest!(b, :meaning)
    assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(a, :routing)
    assert message =~ "has no identity :routing"
  end

  test "set fields keep repeated members" do
    refute Quiddity.digest!(task(tags: ["a", "a"])) == Quiddity.digest!(task(tags: ["a"]))
  end

  defmodule Page do
    @derive Quiddity.Encoder
    defstruct [:number, :size]
  end

  defmodule Query do
    @derive {Quiddity.Encoder, fields: [page: [flatten: true]]}
    defstruct [:term, :page]
  end

  test "flatten writes a struct's members into the containing map" do
    q = %Query{term: "t", page: %Page{number: 2, size: 20}}
    assert Quiddity.digest!(q) == Quiddity.digest!(%{term: "t", number: 2, size: 20})
    assert Quiddity.digest!(%Query{term: "t", page: nil}) == Quiddity.digest!(%{term: "t"})
  end

  defmodule Clash do
    @derive {Quiddity.Encoder, fields: [page: [flatten: true]]}
    defstruct [:number, :page]
  end

  test "a key written twice by a flattened field is an error" do
    assert {:error, %Quiddity.Error{message: message}} =
             Quiddity.digest(%Clash{number: 1, page: %Page{number: 2, size: 3}})

    assert message =~ ~s(the key "number" is written twice)
  end

  defmodule Camel do
    @derive {Quiddity.Encoder, rename_all: :camel_case}
    defstruct [:first_name, :last_name]
  end

  test "rename_all" do
    assert Quiddity.digest!(%Camel{first_name: "A", last_name: "B"}) ==
             Quiddity.digest!(%{"firstName" => "A", "lastName" => "B"})
  end

  test "options are checked when the struct is compiled" do
    for {opts, message} <- [
          {[fields: [missing: [skip: true]]], ":missing is not a field"},
          {[fields: [a: [colour: :red]]], "unknown option for field :a :colour"},
          {[only: [:a], except: [:b]], "cannot be combined"},
          {[fields: [a: [quantize: 0]]], "positive integer"},
          {[fields: [a: [rename: "b"]]], ~s(two fields have the key "b")},
          {[rename_all: :shouting], "rename_all"},
          {[identities: [primary: []]], "primary identity"}
        ] do
      code = """
      defmodule Bad#{System.unique_integer([:positive])} do
        @derive {Quiddity.Encoder, #{inspect(opts)}}
        defstruct [:a, :b]
      end
      """

      error = assert_raise ArgumentError, fn -> Code.eval_string(code) end
      assert Exception.message(error) =~ message
    end
  end

  test "errors say where" do
    assert {:error, %Quiddity.Error{path: "$.question"}} = Quiddity.digest(task(prompt: {1}))

    bad = task(documents: [%DocumentRef{label: "a"}, %DocumentRef{label: {1}}])
    assert {:error, %Quiddity.Error{path: "$.documents[1].label"}} = Quiddity.digest(bad)
  end

  defmodule Reading do
    @derive {Quiddity.Encoder,
             domain: "example.reading",
             fields: [
               price: [kind: :number, decimals: 2],
               at: [kind: :instant, quantize: 1_000_000_000],
               tags: [kind: {:list, :string}, encode_as: :set],
               note: [kind: :string, omit_nil: true],
               ref: [kind: DocumentRef]
             ]}
    @derive JSON.Encoder
    defstruct [:price, :at, :tags, :note, :ref]
  end

  test "a struct exports its identity, which gives its digest to its JSON form" do
    {:ok, identity} = Quiddity.identity(Reading)
    doc = Identity.to_json(identity)
    assert doc["root"] == %{"ref" => "Reading"}
    assert doc["domain"] == "example.reading"
    assert Map.keys(doc["definitions"]) == ["DocumentRef", "Reading"]

    reading = %Reading{
      price: 12.989,
      at: ~U[2026-10-14 09:30:00.250Z],
      tags: ["b", "a"],
      note: nil,
      ref: %DocumentRef{label: "x", content: "y"}
    }

    json = reading |> JSON.encode!() |> JSON.decode!()
    assert Quiddity.digest!(json, identity) == Quiddity.digest!(reading)
  end
end
