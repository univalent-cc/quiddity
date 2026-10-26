defmodule Quiddity.Test.Author do
  @moduledoc false
  @derive [Quiddity.Encoder, JSON.Encoder]
  defstruct [:name]
  @type t :: %__MODULE__{name: String.t()}
end

defmodule Quiddity.Test.Typed do
  @moduledoc false
  @derive [
    {Quiddity.Encoder,
     fields: [tags: [encode_as: :set], price: [decimals: 2], code: [kind: :string]]},
    JSON.Encoder
  ]
  defstruct [:price, :at, :took, :tags, :note, :status, :count, :author, :parent, :meta, :code]

  @type status :: :open | :closed
  @type t :: %__MODULE__{
          price: float(),
          at: DateTime.t(),
          took: Duration.t(),
          tags: [String.t()],
          note: String.t() | nil,
          status: status(),
          count: non_neg_integer(),
          author: Quiddity.Test.Author.t(),
          parent: t() | nil,
          meta: %{optional(String.t()) => term()},
          code: integer()
        }
end
