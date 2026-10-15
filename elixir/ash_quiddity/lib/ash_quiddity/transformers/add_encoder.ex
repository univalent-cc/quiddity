defmodule AshQuiddity.Transformers.AddEncoder do
  @moduledoc false
  # Implements Quiddity.Encoder for the resource's records, and gives the
  # resource module the hook Quiddity reads domains and identities through.
  use Spark.Dsl.Transformer

  @impl true
  def transform(dsl_state) do
    {:ok,
     Spark.Dsl.Transformer.eval(
       dsl_state,
       [],
       quote do
         @doc false
         def __quiddity__({:domain, _identity}), do: AshQuiddity.Info.domain(__MODULE__)
         def __quiddity__({:identity, name}), do: AshQuiddity.Export.identity!(__MODULE__, name)

         defimpl Quiddity.Encoder do
           def encode(record, identity), do: AshQuiddity.Encode.record!(record, identity)
         end
       end
     )}
  end
end
