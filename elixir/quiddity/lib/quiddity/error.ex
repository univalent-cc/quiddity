defmodule Quiddity.Error do
  @moduledoc """
  A value that cannot be digested.

  Returned as `{:error, %Quiddity.Error{}}` by `Quiddity.digest/2`, and
  raised by `Quiddity.digest!/2`. Raised the same way when a value does not
  have the shape an identity describes.

  ## Fields

    * `:path` - where in the value: `"$"` for the value itself, followed by
      `.field` and `[index]` steps.
    * `:message` - what is wrong.
  """

  defexception path: "$", message: nil

  @type t :: %__MODULE__{path: String.t(), message: String.t()}

  @impl true
  def message(%__MODULE__{path: path, message: message}), do: "at #{path}: #{message}"

  @doc false
  def at(path, message), do: %__MODULE__{path: path, message: message}
end

defmodule Quiddity.InvalidIdentity do
  @moduledoc """
  An identity, read or being built, that is not valid (spec section 5.9).

  ## Fields

    * `:path` - where in the identity: `"$"` for the identity itself.
    * `:message` - what is wrong.
  """

  defexception path: "$", message: nil

  @type t :: %__MODULE__{path: String.t(), message: String.t()}

  @impl true
  def message(%__MODULE__{path: path, message: message}),
    do: "invalid identity at #{path}: #{message}"

  @doc false
  def at(path, message), do: %__MODULE__{path: path, message: message}
end
