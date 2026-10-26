defmodule AshQuiddity.Test.Publisher do
  @moduledoc false
  use Ash.Resource, data_layer: :embedded, extensions: [AshQuiddity.Resource]

  attributes do
    attribute(:name, :string, public?: true)
    attribute(:city, :string, public?: true)
    attribute(:contact, :string, public?: true, sensitive?: true)
  end
end

defmodule AshQuiddity.Test.Book do
  @moduledoc false
  use Ash.Resource,
    domain: AshQuiddity.Test.Domain,
    data_layer: Ash.DataLayer.Ets,
    extensions: [AshQuiddity.Resource]

  attributes do
    uuid_primary_key(:id)
    attribute(:title, :string, public?: true, allow_nil?: false)
    attribute(:pages, :integer, public?: true)
    attribute(:genres, {:array, :string}, public?: true, default: [])
    attribute(:binding, :atom, public?: true, constraints: [one_of: [:hardcover, :paperback]])
    attribute(:published_on, :date, public?: true)
    attribute(:catalogued_at, :utc_datetime_usec, public?: true)
    attribute(:loan_period, :duration, public?: true)
    attribute(:price, :decimal, public?: true)
    attribute(:publisher, AshQuiddity.Test.Publisher, public?: true)
    attribute(:shelf_note, :string)
    attribute(:borrower, :string, public?: true, sensitive?: true)
    timestamps()
  end

  calculations do
    calculate(:shout, :string, expr(title <> "!"), public?: true)
    calculate(:etag, :string, {AshQuiddity.Calculation, form: :micro})
    calculate(:meaning_digest, :string, {AshQuiddity.Calculation, identity: :meaning})
  end

  actions do
    default_accept([
      :title,
      :pages,
      :genres,
      :binding,
      :published_on,
      :catalogued_at,
      :loan_period,
      :price,
      :publisher,
      :shelf_note,
      :borrower
    ])

    defaults([:read, :destroy, :create, :update])
  end

  quiddity do
    domain("library.book")
    field(:genres, encode_as: :set)
    field(:catalogued_at, quantize: 1_000_000_000)
    field(:id, skip: true)

    identity :meaning do
      field(:pages, skip: true)
      field(:shout, include: true)
    end
  end
end

defmodule AshQuiddity.Test.Article do
  @moduledoc false
  use Ash.Resource,
    domain: AshQuiddity.Test.Domain,
    data_layer: Ash.DataLayer.Ets,
    extensions: [AshJsonApi.Resource, AshQuiddity.Resource]

  attributes do
    uuid_primary_key(:id)
    attribute(:title_text, :string, public?: true)
    attribute(:body_text, :string, public?: true)
    attribute(:hidden_count, :integer, public?: true)
    attribute(:secret, :string, public?: true, sensitive?: true)
  end

  calculations do
    calculate(:shout, :string, expr(title_text <> "!"), public?: true)
  end

  actions do
    defaults([:read, create: :*])
  end

  json_api do
    type("article")
    field_names(:camelize)
    hide_fields([:hidden_count])
    include_nil_values?(false)
  end

  quiddity do
    follow(:ash_json_api)
  end
end

defmodule AshQuiddity.Test.Domain do
  @moduledoc false
  use Ash.Domain, validate_config_inclusion?: false

  resources do
    resource(AshQuiddity.Test.Book)
    resource(AshQuiddity.Test.Article)
  end
end
