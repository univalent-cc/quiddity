defmodule AshQuiddityTest do
  use ExUnit.Case, async: false
  require Ash.Query
  require Spark.Test

  alias AshQuiddity.Test.{Address, Article, Ticket}
  alias Quiddity.{Digest, Identity}

  defp ticket(overrides \\ %{}) do
    Ticket
    |> Ash.Changeset.for_create(
      :create,
      Map.merge(
        %{
          subject: "Printer jam",
          priority: 1,
          tags: ["hw", "office"],
          status: :open,
          due_on: ~D[2026-10-20],
          asked_at: ~U[2026-10-15 09:30:00.250000Z],
          price: Decimal.new("12.50"),
          address: %{city: "Lyon", zip: "69001", note: "back door"},
          internal_note: "call first",
          secret: "hunter2"
        },
        overrides
      )
    )
    |> Ash.create!()
  end

  # The data-model map of a ticket's public, non-sensitive attributes, as
  # the section configures them.
  defp expected(t) do
    %{
      "subject" => t.subject,
      "priority" => t.priority,
      "tags" => MapSet.new(t.tags),
      "status" => t.status,
      "due_on" => t.due_on,
      "asked_at" => DateTime.truncate(%{t.asked_at | microsecond: {0, 0}}, :second),
      "price" => "12.50",
      "address" => %{"city" => t.address.city, "zip" => t.address.zip}
    }
  end

  test "a record digests as its public, non-sensitive attributes, in its domain" do
    t = ticket()

    assert Quiddity.digest!(t) ==
             t |> expected() |> Quiddity.digest!() |> Digest.in_domain("helpdesk.ticket")
  end

  test "incidental differences leave the digest unchanged" do
    a = ticket()

    b =
      ticket(%{
        tags: ["office", "hw"],
        asked_at: ~U[2026-10-15 09:30:00.900000Z],
        internal_note: "other",
        secret: "other",
        address: %{city: "Lyon", zip: "69001", note: "front door"}
      })

    refute a.id == b.id
    assert Quiddity.digest!(a) == Quiddity.digest!(b)
    refute Quiddity.digest!(a) == Quiddity.digest!(ticket(%{priority: 2}))
  end

  test "an additional identity, with a calculation it includes" do
    a = ticket() |> Ash.load!(:shout)
    b = ticket(%{priority: 9}) |> Ash.load!(:shout)
    refute Quiddity.digest!(a) == Quiddity.digest!(b)
    assert Quiddity.digest!(a, :meaning) == Quiddity.digest!(b, :meaning)

    refute Quiddity.digest!(a, :meaning) ==
             Quiddity.digest!(ticket(%{subject: "x"}) |> Ash.load!(:shout), :meaning)
  end

  test "a field that is not loaded, or forbidden, is an error, not an omission" do
    t = ticket()

    assert {:error, %Quiddity.Error{path: "$.shout", message: message}} =
             Quiddity.digest(t, :meaning)

    assert message =~ "not loaded"

    partial = Ticket |> Ash.Query.select([:subject]) |> Ash.read!() |> hd()
    assert {:error, %Quiddity.Error{path: "$.priority"}} = Quiddity.digest(partial)

    forbidden = %{t | priority: %Ash.ForbiddenField{field: :priority, type: :attribute}}
    assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(forbidden)
    assert message =~ "forbidden"
  end

  test "the digest as a calculation loads what it covers" do
    t = ticket()
    loaded = Ash.load!(t, [:etag, :meaning_digest])
    assert loaded.etag == to_string(Digest.micro(Quiddity.digest!(t)))
    assert loaded.meaning_digest == to_string(Quiddity.digest!(Ash.load!(t, :shout), :meaning))

    # A query that selects only one attribute still gives the digest: the
    # calculation loads the fields it covers.
    [partial] =
      Ticket
      |> Ash.Query.filter(id == ^t.id)
      |> Ash.Query.select([:subject])
      |> Ash.Query.load(:etag)
      |> Ash.read!()

    assert partial.etag == loaded.etag
  end

  test "an embedded resource digests under its own configuration" do
    address = %Address{city: "Lyon", zip: "69001", note: "x"}
    assert Quiddity.digest!(address) == Quiddity.digest!(%{city: "Lyon", zip: "69001"})
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
    t = ticket()
    {:ok, identity} = Quiddity.identity(Ticket)
    doc = Identity.to_json(identity)
    assert doc["domain"] == "helpdesk.ticket"
    assert doc["root"] == %{"ref" => "Ticket"}
    assert Map.has_key?(doc["definitions"], "Address")

    json = %{
      "subject" => "Printer jam",
      "priority" => 1,
      "tags" => ["office", "hw"],
      "status" => "open",
      "due_on" => "2026-10-20",
      "asked_at" => "2026-10-15T09:30:00.250000Z",
      "price" => "12.50",
      "address" => %{"city" => "Lyon", "zip" => "69001"},
      "inserted_at" => "ignored"
    }

    assert Quiddity.digest!(json, identity) == Quiddity.digest!(t)
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
