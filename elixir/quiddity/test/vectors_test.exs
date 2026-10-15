defmodule Quiddity.VectorsTest do
  # The language-neutral test vectors (test-vectors/quiddity-v1.json), which
  # every implementation must reproduce.
  use ExUnit.Case, async: true

  alias Quiddity.{Digest, Identity, Value}

  @path Path.expand("../../../test-vectors/quiddity-v1.json", __DIR__)

  if File.exists?(@path) do
    @vectors (case Quiddity.JSON.decode(File.read!(@path)) do
                {:ok, v} -> v
              end)

    defp vectors, do: @vectors

    # The vectors' typed notation as data-model values.
    defp value(%{"null" => nil}), do: nil
    defp value(%{"bool" => b}), do: b
    defp value(%{"int" => n}), do: String.to_integer(n)
    defp value(%{"string" => s}), do: s
    defp value(%{"code_points" => cps}), do: Identity.Text.wtf8(cps)
    defp value(%{"bytes" => hex}), do: Base.decode16!(hex, case: :lower)
    defp value(%{"list" => items}), do: {:list, Enum.map(items, &value/1)}
    defp value(%{"set" => items}), do: {:set, Enum.map(items, &value/1)}

    defp value(%{"map" => entries}),
      do: {:map, Enum.map(entries, fn [k, v] -> {value(k), value(v)} end)}

    defp value(%{"float" => hex}) do
      <<bits::64>> = Base.decode16!(hex, case: :lower)
      <<sign::1, exp::11, frac::52>> = <<bits::64>>

      cond do
        exp == 0x7FF and frac != 0 ->
          {:float, :nan}

        exp == 0x7FF and sign == 0 ->
          {:float, :infinity}

        exp == 0x7FF ->
          {:float, :neg_infinity}

        true ->
          <<x::float-64>> = <<bits::64>>
          x
      end
    end

    defp hex(bin), do: Base.encode16(bin, case: :lower)

    defp check_digests(digest, case_) do
      assert to_string(digest) == case_["digest"]
      assert to_string(Digest.mini(digest)) == case_["digest128"]
      assert to_string(Digest.micro(digest)) == case_["digest64"]
    end

    test "values" do
      for c <- vectors()["values"] do
        v = value(c["value"])
        assert hex(Value.encode(v)) == c["encoding"], c["name"]
        check_digests(Quiddity.hash(v), c)
      end
    end

    test "domains" do
      for c <- vectors()["domains"] do
        structural = Quiddity.hash(value(c["value"]))
        assert to_string(structural) == c["structural"]
        check_digests(Digest.in_domain(structural, c["label"]), c)
      end
    end

    test "JSON texts" do
      for c <- vectors()["json"] do
        {:ok, term} = Quiddity.JSON.decode(c["text"])
        check_digests(Quiddity.digest!(term), c)
      end
    end

    test "text forms" do
      for c <- vectors()["text_forms"] do
        case {c["valid"], Digest.parse(c["text"])} do
          {true, {:ok, d}} -> assert to_string(d) == String.downcase(c["text"])
          {false, :error} -> :ok
          other -> flunk("#{c["text"]} (#{c["requirement"]}): #{inspect(other)}")
        end
      end
    end

    test "identities" do
      for c <- vectors()["identities"] do
        name = c["name"]
        {:ok, id} = Identity.from_json(c["document"])

        assert to_string(Quiddity.digest!(id)) == c["identity_digest"],
               "identity digest of #{name}"

        assert Identity.from_json(Identity.to_json(id)) == {:ok, id}, "#{name} round-trips"

        for case_ <- c["cases"] do
          source = case_["source"]

          case Quiddity.digest(source, id) do
            {:ok, digest} ->
              refute case_["error"], "#{name}: #{inspect(source)} must fail"

              assert hex(Value.encode(Identity.value!(id, source))) == case_["encoding"],
                     "#{name}: #{inspect(source)}"

              check_digests(digest, case_)

            {:error, e} ->
              assert case_["error"], "#{name}: #{inspect(source)} failed: #{Exception.message(e)}"
          end
        end
      end
    end

    test "invalid documents" do
      for c <- vectors()["invalid_documents"] do
        assert {:error, %Quiddity.InvalidIdentity{}} = Identity.from_json(c["document"]),
               c["name"]
      end
    end

    test "same identity" do
      for c <- vectors()["same_identity"], doc <- c["documents"] do
        {:ok, id} = Identity.from_json(doc)
        assert to_string(Quiddity.digest!(id)) == c["identity_digest"], c["name"]
      end
    end

    test "the identity of identities" do
      c = vectors()["identity_of_identities"]
      {:ok, meta} = Identity.from_json(c["document"])
      assert to_string(Quiddity.digest!(meta)) == c["identity_digest"]
      assert to_string(Quiddity.digest!(Identity.to_json(meta), meta)) == c["identity_digest"]
    end
  else
    @tag skip: "test-vectors/quiddity-v1.json is not present"
    test "vectors", do: :ok
  end
end
