# Digests JSON files under an identity document.
#
#     mix run examples/digest_json.exs examples/data/task.identity.json \
#         examples/data/task.json examples/data/task-reordered.json
#
# With no value files, reads one JSON value from standard input.

defmodule DigestJSON do
  def main([identity_path | value_paths]) do
    identity = Quiddity.Identity.from_json!(File.read!(identity_path))
    IO.puts("#{Quiddity.digest!(identity)}  (identity)")

    inputs =
      case value_paths do
        [] -> [{"-", IO.read(:stdio, :eof)}]
        paths -> Enum.map(paths, &{&1, File.read!(&1)})
      end

    for {name, text} <- inputs do
      case Quiddity.digest(JSON.decode!(text), identity) do
        {:ok, digest} -> IO.puts("#{digest}  #{name}")
        {:error, error} -> IO.puts(:stderr, "#{name}: #{Exception.message(error)}")
      end
    end
  end

  def main([]) do
    IO.puts(:stderr, "usage: mix run examples/digest_json.exs <identity.json> [value.json …]")
    System.halt(1)
  end
end

DigestJSON.main(System.argv())
