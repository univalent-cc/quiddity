defmodule Quiddity.TypespecTest do
  use ExUnit.Case, async: true

  alias Quiddity.Identity
  alias Quiddity.Test.{Author, Typed}

  test "an exported identity takes its kinds from the struct's typespec" do
    {:ok, identity} = Quiddity.identity(Typed)
    doc = Identity.to_json(identity)
    fields = doc["definitions"]["Typed"]["object"]["fields"]

    assert fields["price"] == %{"kind" => "number", "decimals" => 2}
    assert fields["at"] == %{"kind" => %{"instant" => %{}}}
    assert fields["took"] == %{"kind" => %{"duration" => %{}}}
    assert fields["tags"] == %{"kind" => %{"set" => "string"}}
    assert fields["note"] == %{"kind" => %{"union" => ["null", "string"]}}
    assert fields["status"] == %{"kind" => "string"}
    assert fields["count"] == %{"kind" => "integer"}
    assert fields["author"] == %{"kind" => %{"ref" => "Author"}}
    assert fields["parent"] == %{"kind" => %{"union" => ["null", %{"ref" => "Typed"}]}}
    assert fields["meta"] == %{"kind" => %{"map" => %{"key" => "string", "value" => "any"}}}
    # A declared kind takes precedence over the typespec.
    assert fields["code"] == %{"kind" => "string"}
  end

  test "the exported identity gives the struct's digest to its JSON form" do
    typed = %Typed{
      price: 12.989,
      at: ~U[2026-10-15 09:30:00Z],
      took: Duration.new!(minute: 1, second: 30),
      tags: ["b", "a"],
      note: nil,
      status: :open,
      count: 3,
      author: %Author{name: "Ada"},
      parent: %Typed{
        price: 1.5,
        at: ~U[2026-10-14 09:30:00Z],
        took: Duration.new!(day: 2),
        tags: [],
        note: "n",
        status: :closed,
        count: 0,
        author: %Author{name: "Bo"},
        parent: nil,
        meta: %{},
        code: "x"
      },
      meta: %{"k" => [1]},
      code: "y"
    }

    {:ok, identity} = Quiddity.identity(Typed)
    json = typed |> JSON.encode!() |> JSON.decode!()
    assert Quiddity.digest!(json, identity) == Quiddity.digest!(typed)
  end

  defmodule Untyped do
    # Defined in a test file: no compiled module, so no typespec to read.
    @derive Quiddity.Encoder
    defstruct [:a]
    @type t :: %__MODULE__{a: integer()}
  end

  test "without a readable typespec, fields export as any" do
    {:ok, identity} = Quiddity.identity(Untyped)

    assert Identity.to_json(identity)["definitions"]["Untyped"]["object"]["fields"]["a"] == %{
             "kind" => "any"
           }
  end
end
