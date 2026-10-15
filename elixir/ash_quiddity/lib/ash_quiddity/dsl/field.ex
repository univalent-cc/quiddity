defmodule AshQuiddity.Dsl.Field do
  @moduledoc false
  defstruct [
    :name,
    :skip,
    :include,
    :rename,
    :omit_nil,
    :omit_empty,
    :quantize,
    :decimals,
    :encode_as,
    :flatten,
    :identity,
    __spark_metadata__: nil
  ]
end
