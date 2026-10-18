defmodule Quiddity.DeriveTest do
  use ExUnit.Case, async: true

  alias Quiddity.{Digest, Identity}

  defmodule Author do
    @derive [Quiddity.Encoder, JSON.Encoder]
    defstruct [:name, :born]
  end

  defmodule Book do
    @derive {Quiddity.Encoder,
             except: [:last_opened],
             domain: "example.book",
             fields: [
               subtitle: [omit_nil: true],
               genres: [encode_as: :set, omit_empty: true],
               catalogued_at: [quantize: 1_000_000_000],
               price: [decimals: 2],
               title: [rename: "name"]
             ],
             identities: [
               meaning: [fields: [pages: [skip: true]]]
             ]}
    defstruct [:title, :pages, :subtitle, :genres, :catalogued_at, :price, :authors, :last_opened]
  end

  defp book(overrides \\ []) do
    struct!(
      %Book{
        title: "Moby Dick",
        pages: 635,
        subtitle: nil,
        genres: ["novel", "adventure"],
        catalogued_at: ~U[2026-10-14 09:30:00.250Z],
        price: 12.5,
        authors: [%Author{name: "Herman Melville", born: 1819}],
        last_opened: DateTime.utc_now()
      },
      overrides
    )
  end

  test "fields that do not count leave the digest unchanged" do
    base = Quiddity.digest!(book())
    assert Quiddity.digest!(book(last_opened: ~U[2000-01-01 00:00:00Z])) == base
    assert Quiddity.digest!(book(genres: ["adventure", "novel"])) == base
    assert Quiddity.digest!(book(catalogued_at: ~U[2026-10-14 09:30:00.900Z])) == base
    assert Quiddity.digest!(book(price: 12.501)) == base
    refute Quiddity.digest!(book(pages: 720)) == base
  end

  test "the digest is the map it describes, in the struct's domain" do
    b = book(genres: [], price: 12.5)

    expected =
      Quiddity.digest!(%{
        "name" => b.title,
        "pages" => 635,
        "catalogued_at" => 1_791_970_200_000_000_000,
        "price" => 12.5,
        "authors" => [%{"name" => "Herman Melville", "born" => 1819}]
      })
      |> Digest.in_domain("example.book")

    # A nested digest is digested as its text form.
    assert Quiddity.digest!(%{d: Quiddity.digest!("x")}) ==
             Quiddity.digest!(%{d: to_string(Quiddity.digest!("x"))})

    assert Quiddity.digest!(b) == expected
  end

  test "additional identities" do
    a = book(pages: 635)
    b = book(pages: 720)
    refute Quiddity.digest!(a) == Quiddity.digest!(b)
    assert Quiddity.digest!(a, :meaning) == Quiddity.digest!(b, :meaning)
    assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(a, :routing)
    assert message =~ "has no identity :routing"
  end

  test "set fields keep repeated members" do
    refute Quiddity.digest!(book(genres: ["novel", "novel"])) ==
             Quiddity.digest!(book(genres: ["novel"]))
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
    assert {:error, %Quiddity.Error{path: "$.name"}} = Quiddity.digest(book(title: {1}))

    bad = book(authors: [%Author{name: "a"}, %Author{name: {1}}])
    assert {:error, %Quiddity.Error{path: "$.authors[1].name"}} = Quiddity.digest(bad)
  end

  defmodule Meter do
    @derive [Quiddity.Encoder, JSON.Encoder]
    defstruct [:label, :serial]
  end

  defmodule Reading do
    @derive {Quiddity.Encoder,
             domain: "example.reading",
             fields: [
               price: [kind: :number, decimals: 2],
               at: [kind: :instant, quantize: 1_000_000_000],
               tags: [kind: {:list, :string}, encode_as: :set],
               note: [kind: :string, omit_nil: true],
               meter: [kind: Meter]
             ]}
    @derive JSON.Encoder
    defstruct [:price, :at, :tags, :note, :meter]
  end

  test "a struct exports its identity, which gives its digest to its JSON form" do
    {:ok, identity} = Quiddity.identity(Reading)
    doc = Identity.to_json(identity)
    assert doc["root"] == %{"ref" => "Reading"}
    assert doc["domain"] == "example.reading"
    assert Map.keys(doc["definitions"]) == ["Meter", "Reading"]

    reading = %Reading{
      price: 12.989,
      at: ~U[2026-10-14 09:30:00.250Z],
      tags: ["b", "a"],
      note: nil,
      meter: %Meter{label: "x", serial: "y"}
    }

    json = reading |> JSON.encode!() |> JSON.decode!()
    assert Quiddity.digest!(json, identity) == Quiddity.digest!(reading)
  end
end
