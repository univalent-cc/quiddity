defmodule AshQuiddity.Test.Address do
  @moduledoc false
  use Ash.Resource, data_layer: :embedded, extensions: [AshQuiddity.Resource]

  attributes do
    attribute(:city, :string, public?: true)
    attribute(:zip, :string, public?: true)
    attribute(:note, :string, public?: true, sensitive?: true)
  end
end

defmodule AshQuiddity.Test.Ticket do
  @moduledoc false
  use Ash.Resource,
    domain: AshQuiddity.Test.Domain,
    data_layer: Ash.DataLayer.Ets,
    extensions: [AshQuiddity.Resource]

  attributes do
    uuid_primary_key(:id)
    attribute(:subject, :string, public?: true, allow_nil?: false)
    attribute(:priority, :integer, public?: true)
    attribute(:tags, {:array, :string}, public?: true, default: [])
    attribute(:status, :atom, public?: true, constraints: [one_of: [:open, :closed]])
    attribute(:due_on, :date, public?: true)
    attribute(:asked_at, :utc_datetime_usec, public?: true)
    attribute(:price, :decimal, public?: true)
    attribute(:address, AshQuiddity.Test.Address, public?: true)
    attribute(:internal_note, :string)
    attribute(:secret, :string, public?: true, sensitive?: true)
    timestamps()
  end

  calculations do
    calculate(:shout, :string, expr(subject <> "!"), public?: true)
    calculate(:etag, :string, {AshQuiddity.Calculation, form: :micro})
    calculate(:meaning_digest, :string, {AshQuiddity.Calculation, identity: :meaning})
  end

  actions do
    default_accept([
      :subject,
      :priority,
      :tags,
      :status,
      :due_on,
      :asked_at,
      :price,
      :address,
      :internal_note,
      :secret
    ])

    defaults([:read, :destroy, :create, :update])
  end

  quiddity do
    domain("helpdesk.ticket")
    field(:tags, encode_as: :set)
    field(:asked_at, quantize: 1_000_000_000)
    field(:id, skip: true)

    identity :meaning do
      field(:priority, skip: true)
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
    resource(AshQuiddity.Test.Ticket)
    resource(AshQuiddity.Test.Article)
  end
end
