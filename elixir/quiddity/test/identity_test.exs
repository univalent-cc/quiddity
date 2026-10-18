defmodule Quiddity.IdentityTest do
  use ExUnit.Case, async: true

  alias Quiddity.Identity
  import Quiddity.Identity, only: [field: 2]

  test "the README's example" do
    {:ok, identity} =
      Identity.from_json(%{
        "root" => %{
          "object" => %{
            "fields" => %{
              "title" => %{"kind" => "string"},
              "pages" => %{"kind" => "integer"},
              "published" => %{"kind" => %{"instant" => %{}}}
            }
          }
        }
      })

    a =
      Quiddity.digest!(
        %{
          "title" => "Moby Dick",
          "pages" => 635,
          "published" => "1851-10-18T00:00:00Z",
          "shelf" => "B4"
        },
        identity
      )

    b =
      Quiddity.digest!(
        %{"title" => "Moby Dick", "pages" => "635", "published" => "1851-10-18T01:00:00+01:00"},
        identity
      )

    assert a == b

    assert {:error, %Quiddity.Error{path: "$.title"}} =
             Quiddity.digest(
               %{"title" => 5, "pages" => 635, "published" => "1851-10-18T00:00:00Z"},
               identity
             )
  end

  defp row do
    {:object, label: :string, value: :number} |> Identity.new!() |> Identity.named("Row")
  end

  defp question do
    shape =
      {:tagged,
       [
         Empty: nil,
         Rect: {:object, w: :integer, h: :integer}
       ], style: :internal, tag: "type"}

    {:object,
     brief: :string,
     note: field(:string, optional: true),
     tags: field({:set, :string}, omit_empty: true),
     rows: {:list, row()},
     best: field(row(), optional: true),
     parent: field(:recursive, optional: true),
     kind: field(:string, rename: "type"),
     shape: shape}
    |> Identity.new!(domain: "example.question")
    |> Identity.named("Question")
  end

  test "a composed identity matches its JSON form" do
    q = question()
    doc = Identity.to_json(q)
    assert doc["root"] == %{"ref" => "Question"}
    assert Map.keys(doc["definitions"]) == ["Question", "Row"]
    fields = doc["definitions"]["Question"]["object"]["fields"]
    assert fields["rows"]["kind"] == %{"list" => %{"ref" => "Row"}}
    assert fields["parent"]["kind"] == %{"union" => ["null", %{"ref" => "Question"}]}
    assert fields["tags"]["omit"] == ["empty", "none"]

    {:ok, loaded} = Identity.from_json(doc)
    assert loaded == q
    assert Quiddity.digest!(loaded) == Quiddity.digest!(q)

    {:ok, from_text} = doc |> JSON.encode!() |> Identity.from_json()
    assert from_text == q

    value = %{
      brief: "b",
      rows: [%{label: "x", value: 1}],
      kind: "q",
      shape: %{type: "Rect", w: 1, h: 2},
      parent: %{brief: "p", rows: [], kind: "q", shape: %{type: "Empty"}}
    }

    assert Quiddity.digest!(value, q) == Quiddity.digest!(value, loaded)
  end

  test "definitions as identities" do
    q = question()
    assert Identity.definition(q, "Row") == row()
    assert Identity.name(Identity.definition(q, "Row")) == "Row"
    assert Identity.definition(q, "Missing") == nil
  end

  test "different identities with one name are both kept" do
    other = {:object, id: :integer} |> Identity.new!() |> Identity.named("Row")
    id = Identity.new!({:object, a: row(), b: other})
    assert Map.keys(Identity.to_json(id)["definitions"]) == ["Row", "Row_2"]
  end

  test "names are in the JSON form, and renaming renames references" do
    doc = Identity.to_json(row())
    assert doc["root"] == %{"ref" => "Row"}
    {:ok, back} = Identity.from_json(doc)
    assert Identity.name(back) == "Row"

    renamed = Identity.named(question(), "Query")
    doc = Identity.to_json(renamed)
    assert doc["root"] == %{"ref" => "Query"}
    parent = doc["definitions"]["Query"]["object"]["fields"]["parent"]
    assert parent["kind"] == %{"union" => ["null", %{"ref" => "Query"}]}
    refute Quiddity.digest!(renamed) == Quiddity.digest!(question())
  end

  test "unnamed identities are written in place" do
    point = Identity.new!({:object, x: :integer})
    id = Identity.new!({:list, point})

    assert Identity.to_json(id)["root"] == %{
             "list" => %{"object" => %{"fields" => %{"x" => %{"kind" => "integer"}}}}
           }
  end

  test "changing fields" do
    q = question()
    {:ok, q2} = Identity.remove_field_in(q, "Question", "rows")
    assert Map.has_key?(Identity.to_json(q2)["definitions"], "Row")
    {:ok, q2} = Identity.remove_field_in(q2, "Question", "best")
    refute Map.has_key?(Identity.to_json(q2)["definitions"] || %{}, "Row")
    {:ok, q2} = Identity.set_field_in(q2, "Question", "rows", {:list, row()})
    assert Map.has_key?(Identity.to_json(q2)["definitions"], "Row")

    node = {:object, value: :integer} |> Identity.new!() |> Identity.named("Node")
    {:ok, node} = Identity.set_field(node, :children, {:list, :recursive})
    assert Identity.to_json(node)["root"] == %{"ref" => "Node"}
    tree = %{value: 1, children: [%{value: 2, children: []}]}
    assert {:ok, _} = Quiddity.digest(tree, node)

    plain = Identity.new!({:object, value: :integer})
    {:ok, recursive} = Identity.set_field(plain, :next, field(:recursive, optional: true))
    assert Identity.name(recursive) == "_self"
  end

  test "a change that would be invalid changes nothing" do
    q = question()

    assert {:error, %Quiddity.InvalidIdentity{}} =
             Identity.set_field_in(q, "Question", "bad", {:union, [:integer, :number]})

    assert {:error, _} =
             Identity.set_field_in(q, "Question", "other", field(:string, rename: "brief"))

    assert {:error, _} = Identity.set_field_in(q, "Missing", "x", :string)
    assert {:error, _} = Identity.set_domain(q, "")
    assert {:ok, without} = Identity.set_domain(q, nil)
    assert Identity.domain(without) == nil
  end

  test "constructors check what they build" do
    assert {:error, _} = Identity.new({:union, [:integer, :number]})
    assert {:error, _} = Identity.new({:union, []})
    assert {:error, _} = Identity.new({:tuple, []})
    assert {:error, _} = Identity.new({:map, {:list, :string}, :any})
    assert {:error, _} = Identity.new({:object, [a: :string, a: :integer]})

    assert {:error, _} =
             Identity.new({:object, [a: field(:string, optional: true)], as_list: true})

    assert {:error, _} = Identity.new({:tagged, [A: :string], style: :internal, tag: "t"})

    assert {:error, _} =
             Identity.new({:tagged, [A: nil], style: :adjacent, tag: "t", content: "t"})

    assert {:error, _} = Identity.new(:recursive)
    assert {:error, _} = Identity.new(:colour)
    assert {:ok, _} = Identity.new({:union, [:null, :string]})
  end

  test "tagged unions by discriminant" do
    level =
      Identity.new!(
        {:tagged,
         [
           Low: Identity.variant(nil, discriminant: 0),
           High: Identity.variant(nil, discriminant: 1)
         ], encode_as: :int}
      )

    assert Quiddity.digest!("High", level) == Quiddity.digest!(1, Identity.new!(:integer))
  end

  test "quantization" do
    reading =
      Identity.new!(
        {:object,
         price: field(:number, decimals: 2),
         count: field(:integer, quantize: 100),
         at: field(:instant, quantize: 1_000_000_000)}
      )

    a = Quiddity.digest!(%{price: 12.989, count: 149, at: "2024-01-01T00:00:00.75Z"}, reading)
    b = Quiddity.digest!(%{price: 12.98, count: 100, at: ~U[2024-01-01 00:00:00Z]}, reading)
    assert a == b
    refute a == Quiddity.digest!(%{price: 12.98, count: 99, at: "2024-01-01T00:00:00Z"}, reading)
  end

  test "an identity encodes to JSON as its JSON form" do
    q = question()
    assert JSON.decode!(JSON.encode!(q)) == JSON.decode!(JSON.encode!(Identity.to_json(q)))
    assert Jason.decode!(Jason.encode!(q)) == JSON.decode!(JSON.encode!(Identity.to_json(q)))
  end

  test "JSON with repeated member names, or that is not JSON, is an error" do
    assert {:error, %Quiddity.InvalidIdentity{message: message}} =
             Identity.from_json(~s({"root": "string", "root": "integer"}))

    assert message =~ "repeated"
    assert {:error, _} = Identity.from_json("{")

    assert_raise Quiddity.InvalidIdentity, fn ->
      Identity.from_json!(~s({"quiddity": 1, "root": "string"}))
    end

    assert_raise Quiddity.InvalidIdentity, ~r/quantize/, fn ->
      Identity.from_json!(
        ~s({"root": {"object": {"fields": {"n": {"kind": "integer", "quantize": 5.0}}}}})
      )
    end
  end

  # The Rust crate's examples/dynamic_identity.rs builds the same identity
  # and prints these digests.
  test "agrees with the Rust crate" do
    author =
      {:object, name: :string, born: :integer}
      |> Identity.new!()
      |> Identity.named("Author")

    format =
      {:tagged, [Ebook: nil, Print: {:object, volumes: :integer}], style: :internal, tag: "type"}

    book =
      {:object,
       title: :string,
       authors: {:list, author},
       pages: :integer,
       format: format,
       subtitle: field(:string, optional: true),
       genres: field({:set, :string}, omit_empty: true),
       cataloguedAt: field(:instant, rename: "catalogued_at", quantize: 1_000_000_000),
       follows: field(:recursive, optional: true)}
      |> Identity.new!(domain: "example.book")
      |> Identity.named("Book")

    value = %{
      "title" => "Moby Dick",
      "authors" => [%{"name" => "Herman Melville", "born" => 1819}],
      "pages" => 635,
      "format" => %{"type" => "Print", "volumes" => 3},
      "genres" => ["novel", "adventure"],
      "cataloguedAt" => "2026-10-14T09:30:00.250Z",
      "last_opened" => "2026-10-16T21:05:00Z"
    }

    assert to_string(Quiddity.digest!(value, book)) ==
             "sha256:cadb496f7865e4fea62732c25f187c400b16e707a5f32721d56253dd1ef0d867"

    typee = %{value | "title" => "Typee", "pages" => 298}

    assert to_string(Quiddity.digest!(Map.put(value, "follows", typee), book)) ==
             "sha256:a4b74427b4d91929350c175b5ec6f0455d0fcaec9215486d58fb93021623ae64"

    {:ok, book} = Identity.remove_field_in(book, "Book", "pages")

    assert to_string(Quiddity.digest!(value, book)) ==
             "sha256:bfe610c85a00edf4f1839547de54986b056346540c734d9f31de8eb6eee4a3e0"

    assert to_string(Quiddity.digest!(book)) ==
             "sha256:0c0c0a3d2307dfd21e5ae5acbec47e4e9228d0e54fbafd0061f2060c153dde8a"
  end
end
