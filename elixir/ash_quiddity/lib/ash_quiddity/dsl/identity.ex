defmodule AshQuiddity.Dsl.Identity do
  @moduledoc false
  defstruct [:name, :follow, :only, :except, fields: [], __spark_metadata__: nil]
end
