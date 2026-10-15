defmodule AshQuiddity.Verifiers.VerifyFields do
  @moduledoc false
  # Checks the `quiddity` section against the resource once it is compiled:
  # every field it names exists, identities have distinct names, and fields
  # the followed encoder names are not renamed. Verifiers run after
  # compilation, so they add no compile-time dependencies between resources.
  use Spark.Dsl.Verifier

  alias AshQuiddity.Dsl.{Field, Identity}
  alias Spark.Dsl.Verifier

  @impl true
  def verify(dsl_state) do
    resource = Verifier.get_persisted(dsl_state, :module)
    entities = Verifier.get_entities(dsl_state, [:quiddity])
    identities = Enum.filter(entities, &match?(%Identity{}, &1))
    section_follow = Verifier.get_option(dsl_state, [:quiddity], :follow)
    known = known_fields(dsl_state)

    with :ok <- unique_identities(resource, identities),
         :ok <-
           check_fields(
             resource,
             Enum.filter(entities, &match?(%Field{}, &1)),
             section_follow,
             known
           ) do
      Enum.reduce_while(identities, :ok, fn id, :ok ->
        case check_fields(resource, id.fields, id.follow || section_follow, known) do
          :ok -> {:cont, :ok}
          error -> {:halt, error}
        end
      end)
    end
  end

  defp known_fields(dsl_state) do
    for section <- [[:attributes], [:calculations], [:aggregates], [:relationships]],
        entity <- Verifier.get_entities(dsl_state, section),
        name = Map.get(entity, :name),
        do: name
  end

  defp unique_identities(resource, identities) do
    names = Enum.map(identities, & &1.name)

    cond do
      :primary in names ->
        error(resource, "the primary identity is the section itself; it cannot be declared")

      names -- Enum.uniq(names) != [] ->
        error(resource, "two identities have one name")

      true ->
        :ok
    end
  end

  defp check_fields(resource, fields, follow, known) do
    Enum.reduce_while(fields, :ok, fn f, :ok ->
      cond do
        f.name not in known ->
          {:halt, error(resource, "#{inspect(f.name)} is not a field of the resource")}

        follow == :ash_json_api and f.rename != nil ->
          {:halt,
           error(
             resource,
             "#{inspect(f.name)}: `rename` cannot be used when following :ash_json_api, whose " <>
               "`field_names` names the fields"
           )}

        f.quantize != nil and f.decimals != nil ->
          {:halt, error(resource, "#{inspect(f.name)} has quantize or decimals, not both")}

        true ->
          {:cont, :ok}
      end
    end)
  end

  defp error(resource, message) do
    {:error,
     Spark.Error.DslError.exception(module: resource, path: [:quiddity], message: message)}
  end
end
