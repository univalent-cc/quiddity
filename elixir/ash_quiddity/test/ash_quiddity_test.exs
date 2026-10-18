defmodule AshQuiddityTest do
  use ExUnit.Case, async: false
  require Ash.Query
  require Spark.Test

  alias AshQuiddity.Test.{Article, Book, Publisher}
  alias Quiddity.{Digest, Identity}

  defp book(overrides \\ %{}) do
    Book
    |> Ash.Changeset.for_create(
      :create,
      Map.merge(
        %{
          title: "Moby Dick",
          pages: 635,
          genres: ["novel", "adventure"],
          binding: :hardcover,
          published_on: ~D[1851-10-18],
          catalogued_at: ~U[2026-10-15 09:30:00.250000Z],
          price: Decimal.new("12.50"),
          publisher: %{name: "Harper & Brothers", city: "New York", contact: "front desk"},
          shelf_note: "spine is loose",
          borrower: "Ishmael"
        },
        overrides
      )
    )
    |> Ash.create!()
  end

  # The data-model map of a book's public, non-sensitive attributes, as
  # the section configures them.
  defp expected(b) do
    %{
      "title" => b.title,
      "pages" => b.pages,
      "genres" => MapSet.new(b.genres),
      "binding" => b.binding,
      "published_on" => b.published_on,
      "catalogued_at" => DateTime.truncate(%{b.catalogued_at | microsecond: {0, 0}}, :second),
      "price" => "12.50",
      "publisher" => %{"name" => b.publisher.name, "city" => b.publisher.city}
    }
  end

  test "a record digests as its public, non-sensitive attributes, in its domain" do
    b = book()

    assert Quiddity.digest!(b) ==
             b |> expected() |> Quiddity.digest!() |> Digest.in_domain("library.book")
  end

  test "incidental differences leave the digest unchanged" do
    a = book()

    b =
      book(%{
        genres: ["adventure", "novel"],
        catalogued_at: ~U[2026-10-15 09:30:00.900000Z],
        shelf_note: "returned late",
        borrower: "Queequeg",
        publisher: %{name: "Harper & Brothers", city: "New York", contact: "the editor"}
      })

    refute a.id == b.id
    assert Quiddity.digest!(a) == Quiddity.digest!(b)
    refute Quiddity.digest!(a) == Quiddity.digest!(book(%{pages: 720}))
  end

  test "an additional identity, with a calculation it includes" do
    a = book() |> Ash.load!(:shout)
    b = book(%{pages: 720}) |> Ash.load!(:shout)
    refute Quiddity.digest!(a) == Quiddity.digest!(b)
    assert Quiddity.digest!(a, :meaning) == Quiddity.digest!(b, :meaning)

    refute Quiddity.digest!(a, :meaning) ==
             Quiddity.digest!(book(%{title: "Typee"}) |> Ash.load!(:shout), :meaning)
  end

  test "a field that is not loaded, or forbidden, is an error, not an omission" do
    b = book()

    assert {:error, %Quiddity.Error{path: "$.shout", message: message}} =
             Quiddity.digest(b, :meaning)

    assert message =~ "not loaded"

    partial = Book |> Ash.Query.select([:title]) |> Ash.read!() |> hd()
    assert {:error, %Quiddity.Error{path: "$.pages"}} = Quiddity.digest(partial)

    forbidden = %{b | pages: %Ash.ForbiddenField{field: :pages, type: :attribute}}
    assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(forbidden)
    assert message =~ "forbidden"
  end

  test "the digest as a calculation loads what it covers" do
    b = book()
    loaded = Ash.load!(b, [:etag, :meaning_digest])
    assert loaded.etag == to_string(Digest.micro(Quiddity.digest!(b)))
    assert loaded.meaning_digest == to_string(Quiddity.digest!(Ash.load!(b, :shout), :meaning))

    # A query that selects only one attribute still gives the digest: the
    # calculation loads the fields it covers.
    [partial] =
      Book
      |> Ash.Query.filter(id == ^b.id)
      |> Ash.Query.select([:title])
      |> Ash.Query.load(:etag)
      |> Ash.read!()

    assert partial.etag == loaded.etag
  end

  test "an embedded resource digests under its own configuration" do
    publisher = %Publisher{name: "Harper & Brothers", city: "New York", contact: "x"}

    assert Quiddity.digest!(publisher) ==
             Quiddity.digest!(%{name: "Harper & Brothers", city: "New York"})
  end

  test "following AshJsonApi: its fields, names, primary key and nil values" do
    a =
      Article
      |> Ash.Changeset.for_create(:create, %{
        title_text: "Hi",
        body_text: nil,
        hidden_count: 3,
        secret: "s"
      })
      |> Ash.create!()

    expected =
      Quiddity.digest!(%{"id" => a.id, "titleText" => "Hi", "secret" => "s"})

    assert Quiddity.digest!(a) == expected
  end

  test "a resource exports its identity, which gives a record's digest to its JSON form" do
    b = book()
    {:ok, identity} = Quiddity.identity(Book)
    doc = Identity.to_json(identity)
    assert doc["domain"] == "library.book"
    assert doc["root"] == %{"ref" => "Book"}
    assert Map.has_key?(doc["definitions"], "Publisher")

    json = %{
      "title" => "Moby Dick",
      "pages" => 635,
      "genres" => ["adventure", "novel"],
      "binding" => "hardcover",
      "published_on" => "1851-10-18",
      "catalogued_at" => "2026-10-15T09:30:00.250000Z",
      "price" => "12.50",
      "publisher" => %{"name" => "Harper & Brothers", "city" => "New York"},
      "inserted_at" => "ignored"
    }

    assert Quiddity.digest!(json, identity) == Quiddity.digest!(b)
  end

  test "the section is checked when the resource is compiled" do
    # Verifiers run after compilation; `mix compile` fails on their errors,
    # and Spark.Test collects them.
    errors =
      Spark.Test.dsl_errors do
        defmodule Elixir.AshQuiddity.Test.BadField do
          use Ash.Resource, domain: nil, extensions: [AshQuiddity.Resource]

          attributes do
            uuid_primary_key(:id)
            attribute(:title, :string, public?: true)
          end

          quiddity do
            field(:missing, skip: true)
          end
        end

        defmodule Elixir.AshQuiddity.Test.BadIdentity do
          use Ash.Resource, domain: nil, extensions: [AshQuiddity.Resource]

          attributes do
            uuid_primary_key(:id)
          end

          quiddity do
            identity :primary do
            end
          end
        end

        defmodule Elixir.AshQuiddity.Test.BadRename do
          use Ash.Resource, domain: nil, extensions: [AshJsonApi.Resource, AshQuiddity.Resource]

          attributes do
            uuid_primary_key(:id)
            attribute(:title, :string, public?: true)
          end

          json_api do
            type("bad")
          end

          quiddity do
            follow(:ash_json_api)
            field(:title, rename: "t")
          end
        end
      end

    messages = for {_module, errors} <- errors, e <- errors, do: Exception.message(e)
    assert Enum.any?(messages, &(&1 =~ ":missing is not a field"))
    assert Enum.any?(messages, &(&1 =~ "primary identity"))
    assert Enum.any?(messages, &(&1 =~ "rename"))
  end
end
