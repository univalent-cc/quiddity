defmodule QuiddityTest do
  use ExUnit.Case, async: true

  doctest Quiddity
  doctest Quiddity.Value
  doctest Quiddity.Digest
  doctest Quiddity.Identity

  alias Quiddity.{Digest, Value}

  describe "Elixir values" do
    test "map keys in any order, atoms or strings" do
      assert Quiddity.digest!(%{a: 1, b: 2}) == Quiddity.digest!(%{"b" => 2, "a" => 1})
    end

    test "numbers by value" do
      assert Quiddity.digest!(5) == Quiddity.digest!(5.0)
      assert Quiddity.digest!(0) == Quiddity.digest!(-0.0)
      refute Quiddity.digest!(true) == Quiddity.digest!(1)
    end

    test "atoms are their names" do
      assert Quiddity.digest!(:ok) == Quiddity.digest!("ok")
      assert Quiddity.digest!(nil) != Quiddity.digest!("nil")
    end

    test "a DateTime is an instant, whatever its time zone" do
      utc = ~U[2024-01-01 00:00:00Z]
      shifted = DateTime.add(utc, 0, :second) |> Map.put(:microsecond, {0, 6})
      assert Value.from(utc) == {:instant, 1_704_067_200_000_000_000}
      assert Quiddity.digest!(utc) == Quiddity.digest!(shifted)
      assert Quiddity.digest!(utc) == Quiddity.digest!(1_704_067_200_000_000_000)
    end

    test "dates, times and durations as ISO 8601 text" do
      assert Value.from(~D[2026-10-15]) == "2026-10-15"
      assert Value.from(~N[2026-10-15 01:02:03]) == "2026-10-15T01:02:03"
      assert Value.from(~T[01:02:03]) == "01:02:03"
      assert Value.from(Duration.new!(month: 1, second: 2)) == "P1MT2S"
    end

    test "sets are unordered, and are not lists" do
      assert Quiddity.digest!(MapSet.new([1, 2])) == Quiddity.digest!(MapSet.new([2, 1]))
      refute Quiddity.digest!(MapSet.new([1, 2])) == Quiddity.digest!([1, 2])
    end

    test "what JSON cannot write is an error" do
      assert {:error, %Quiddity.Error{message: "a tuple" <> _}} = Quiddity.digest({1, 2})
      assert {:error, %Quiddity.Error{path: "$[0]"}} = Quiddity.digest(a: 1)
      assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(%{:a => 1, "a" => 2})
      assert message =~ "given twice"
      assert {:error, %Quiddity.Error{message: message}} = Quiddity.digest(URI.parse("x"))
      assert message =~ "does not implement Quiddity.Encoder"
      assert_raise Quiddity.Error, fn -> Quiddity.digest!(%{a: [{1, 2}]}) end
    end

    test "errors say where" do
      assert {:error, %Quiddity.Error{path: "$.a[1]"}} = Quiddity.digest(%{a: [1, {2}]})
    end
  end

  describe "digests" do
    test "agree with the Rust crate's README" do
      digest = Quiddity.digest!("Summarize the report")
      assert to_string(Digest.micro(digest)) == "sha256-64:366d2ab7f2c3e7db"
    end

    test "text forms round-trip, and encode to JSON as text" do
      d = Quiddity.digest!([1, 2])
      assert Digest.parse!(to_string(d)) == d
      assert JSON.encode!(%{etag: d}) == ~s({"etag":"#{d}"})
      assert Jason.encode!(%{etag: d}) == ~s({"etag":"#{d}"})
      assert inspect(d) == "#Quiddity.Digest<#{d}>"
    end

    test "domains" do
      d = Quiddity.digest!(%{to: "bob", amount: 100})
      refute Digest.in_domain(d, "example.payment") == Digest.in_domain(d, "example.refund")
      assert_raise ArgumentError, fn -> Digest.in_domain(d, "") end
      assert_raise ArgumentError, fn -> Digest.in_domain(Digest.micro(d), "x") end
    end
  end
end
