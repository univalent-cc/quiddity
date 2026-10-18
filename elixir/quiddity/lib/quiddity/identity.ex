defmodule Quiddity.Identity do
  @moduledoc """
  An identity as data (spec section 5): which parts of a value are digested,
  and how each is read.

  An identity is built in code with `new/2`, or read from its JSON form,
  which every implementation of quiddity reads and writes, with
  `from_json/1`. `Quiddity.digest/2` applies it to any value: a map decoded
  from JSON, a struct, an Ash record.

      iex> {:ok, book} =
      ...>   Quiddity.Identity.new(
      ...>     {:object, title: :string, pages: :integer, published: :instant}
      ...>   )
      iex> a = Quiddity.digest!(%{"title" => "Moby Dick", "pages" => 635, "published" => "1851-10-18T00:00:00Z", "shelf" => "B4"}, book)
      iex> b = Quiddity.digest!(%{title: "Moby Dick", pages: "635", published: "1851-10-18T01:00:00+01:00"}, book)
      iex> a == b
      true

  Members the identity does not list, such as `"shelf"`, are left out.
  Values written in other forms are converted: the text `"635"` to an
  integer, RFC 3339 text to an instant. A value of the wrong shape is an
  error.

  An identity is always valid and in canonical form (spec 5.3, 5.9): every
  function that builds or changes one checks it, and returns
  `{:error, %Quiddity.InvalidIdentity{}}`, changing nothing, when the
  result would not be valid.

  ## Kinds

  Kinds are Elixir terms in the shape of an identity's JSON form:

  | Kind | Reads |
  |---|---|
  | `:null`, `:bool`, `:integer`, `:number`, `:string`, `:uuid`, `:path`, `:any` | as named; `:any` takes any value as it is |
  | `:bytes`, `{:bytes, text: :hex}` | bytes: a list of integers, or base64 (or hex) text |
  | `:instant`, `{:instant, unit: :ms}` | an instant: a `DateTime`, RFC 3339 text, or a number in the unit |
  | `:duration`, `{:duration, unit: :s}` | a duration: ISO 8601 text, or a number in the unit |
  | `{:list, kind}`, `{:set, kind}` | a list; a set, in which order does not count |
  | `{:tuple, [kind, ...]}` | a fixed-length list |
  | `{:map, key_kind, value_kind}` | a map |
  | `{:optional, kind}` | `kind` or `nil` |
  | `{:union, [kind, ...]}` | one of several kinds, told apart by the value's type |
  | `{:object, fields}`, `{:object, fields, as_list: true}` | an object with these fields, as a keyword list or map of names and kinds |
  | `{:tagged, variants, opts}` | one of several named variants |
  | a `%Quiddity.Identity{}` | another identity, nested |
  | `:recursive` | the identity being built |

  A field's kind can carry options, as `field/2` builds them: `rename`,
  `omit`, `optional`, `omit_empty`, `quantize` and `decimals`. A
  variant can carry `rename` and `discriminant`, as `variant/2` builds
  them. A tagged union's options are `style` (`:external`, the default,
  `:internal` or `:adjacent`), `tag`, `content`, and `encode_as: :int`.
  """

  alias Quiddity.{InvalidIdentity, Value}
  alias Quiddity.Identity.{Apply, Load, Meta}

  @enforce_keys [:root]
  defstruct domain: nil, root: nil, definitions: %{}

  @typedoc "An identity: always valid, in canonical form."
  @type t :: %__MODULE__{domain: String.t() | nil, root: map | String.t(), definitions: map}

  @typedoc "A kind, as described in the module documentation."
  @type kind :: atom | tuple | t

  # --- Building ---------------------------------------------------------------

  @doc """
  An identity whose root is `kind`.

  ## Options

    * `:domain` - a label hashed into every digest under the identity
      (spec 4.2).

  ## Examples

      iex> {:ok, id} = Quiddity.Identity.new({:list, :string})
      iex> Quiddity.Identity.to_json(id)
      %{"root" => %{"list" => "string"}}

      iex> Quiddity.Identity.new({:union, [:integer, :number]})
      {:error, %Quiddity.InvalidIdentity{path: "$.root", message: ~s(two union alternatives accept ["number"] natively)}}
  """
  @spec new(kind, keyword) :: {:ok, t} | {:error, InvalidIdentity.t()}
  def new(kind, opts \\ []), do: catching(fn -> new!(kind, opts) end)

  @doc "An identity whose root is `kind`, as `new/2`, raising `Quiddity.InvalidIdentity`."
  @spec new!(kind, keyword) :: t
  def new!(kind, opts \\ []) do
    domain = Keyword.get(opts, :domain)
    doc = to_doc(kind, "$.root")
    {root, defs} = absorb_root(doc, %{})
    build!(root, defs, domain)
  end

  @doc """
  A field's kind with options, for an `{:object, fields}` kind.

  ## Options

    * `:rename` - the key it is digested under; by default its name.
    * `:optional` - when `true`, the field may be `nil` or missing, and is
      left out then (`omit: [:none]` with `{:optional, kind}`).
    * `:omit_empty` - when `true`, the field may be `nil`, missing or empty,
      and is left out then.
    * `:omit` - conditions under which it is left out: `:none` (`nil` or
      missing) and `:empty` (an empty string, list, set or map).
    * `:quantize` - round an integer, instant or duration down to a multiple
      of this step (nanoseconds for times).
    * `:decimals` - round a number down to this many decimal places.

  ## Examples

      iex> {:ok, id} = Quiddity.Identity.new({:object, subtitle: Quiddity.Identity.field(:string, optional: true)})
      iex> Quiddity.digest!(%{}, id) == Quiddity.digest!(%{"subtitle" => nil}, id)
      true
  """
  @spec field(kind, keyword) :: {:field, kind, keyword}
  def field(kind, opts \\ []), do: {:field, kind, opts}

  @doc """
  A variant of a tagged union with options: `:rename` and, for
  `encode_as: :int`, `:discriminant`.
  """
  @spec variant(kind | nil, keyword) :: {:variant, kind | nil, keyword}
  def variant(kind, opts \\ []), do: {:variant, kind, opts}

  @doc """
  The identity, named `name`: its root becomes its own definition under
  that name, which is how it appears in JSON forms, its own and those of
  identities it is nested in. A nested identity without a name is written
  in place.

  ## Examples

      iex> row = {:object, label: :string} |> Quiddity.Identity.new!() |> Quiddity.Identity.named("Row")
      iex> table = Quiddity.Identity.new!({:list, row})
      iex> Quiddity.Identity.to_json(table)["root"]
      %{"list" => %{"ref" => "Row"}}
  """
  @spec named(t, String.t()) :: t
  def named(%__MODULE__{} = id, name) when is_binary(name) and name != "" do
    case id.root do
      %{"ref" => own} ->
        if own == name do
          id
        else
          defs = id.definitions |> make_room(name, [own]) |> rename_def(own, name)
          %{id | root: %{"ref" => name}, definitions: defs}
        end

      root ->
        defs = make_room(id.definitions, name, [])
        %{id | root: %{"ref" => name}, definitions: Map.put(defs, name, root)}
    end
  end

  @doc """
  The name of the identity's own definition, if its root is one: the name
  given with `named/2`, or `"_self"` for an unnamed recursive identity.
  """
  @spec name(t) :: String.t() | nil
  def name(%__MODULE__{root: %{"ref" => name}}), do: name
  def name(%__MODULE__{}), do: nil

  @doc "The identity's domain, if it has one."
  @spec domain(t) :: String.t() | nil
  def domain(%__MODULE__{domain: domain}), do: domain

  @doc """
  The identity with the domain `label`, or without a domain for `nil`.
  """
  @spec set_domain(t, String.t() | nil) :: {:ok, t} | {:error, InvalidIdentity.t()}
  def set_domain(%__MODULE__{} = id, nil), do: {:ok, %{id | domain: nil}}

  def set_domain(%__MODULE__{} = id, label) do
    catching(fn -> build!(id.root, id.definitions, label) end)
  end

  @doc """
  The definition `name` of the identity, as an identity of its own.
  """
  @spec definition(t, String.t()) :: t | nil
  def definition(%__MODULE__{definitions: defs}, name) do
    if Map.has_key?(defs, name) do
      {_domain, root, defs} = Load.build!(%{"ref" => name}, defs, nil)
      %__MODULE__{root: root, definitions: defs}
    end
  end

  @doc """
  The identity with the field `name` of its root object set to `field`, a
  kind or `field/2`. The root may be an object, or the identity's own
  definition when it is one.

  ## Examples

      iex> id = Quiddity.Identity.new!({:object, title: :string})
      iex> {:ok, id} = Quiddity.Identity.set_field(id, :pages, :integer)
      iex> Quiddity.Identity.to_json(id)["root"]["object"]["fields"] |> Map.keys()
      ["pages", "title"]
  """
  @spec set_field(t, atom | String.t(), kind | tuple) :: {:ok, t} | {:error, InvalidIdentity.t()}
  def set_field(%__MODULE__{} = id, name, field) do
    catching(fn ->
      {target, id} = own_object!(id, field)

      edit_object!(id, target, fn fields, defs -> put_field(fields, defs, name, field, target) end)
    end)
  end

  @doc "The identity without the field `name` of its root object."
  @spec remove_field(t, atom | String.t()) :: {:ok, t} | {:error, InvalidIdentity.t()}
  def remove_field(%__MODULE__{} = id, name) do
    catching(fn ->
      {target, id} = own_object!(id, nil)
      edit_object!(id, target, fn fields, defs -> {Map.delete(fields, to_string(name)), defs} end)
    end)
  end

  @doc "The identity with the field `name` of its object definition `definition` set to `field`."
  @spec set_field_in(t, String.t(), atom | String.t(), kind | tuple) ::
          {:ok, t} | {:error, InvalidIdentity.t()}
  def set_field_in(%__MODULE__{} = id, definition, name, field) do
    catching(fn ->
      edit_object!(id, {:def, definition}, fn fields, defs ->
        put_field(fields, defs, name, field, {:def, definition})
      end)
    end)
  end

  @doc "The identity without the field `name` of its object definition `definition`."
  @spec remove_field_in(t, String.t(), atom | String.t()) ::
          {:ok, t} | {:error, InvalidIdentity.t()}
  def remove_field_in(%__MODULE__{} = id, definition, name) do
    catching(fn ->
      edit_object!(id, {:def, definition}, fn fields, defs ->
        {Map.delete(fields, to_string(name)), defs}
      end)
    end)
  end

  # --- JSON forms -------------------------------------------------------------

  @doc """
  Reads an identity from its JSON form (spec 5.3): JSON text, or a map
  decoded from it. An object with a repeated member name is an error, and
  so is a number where an integer is required, such as `5.0`.

  ## Examples

      iex> {:ok, id} = Quiddity.Identity.from_json(~s({"root": "string"}))
      iex> id == Quiddity.Identity.new!(:string)
      true

      iex> {:error, %Quiddity.InvalidIdentity{message: message}} =
      ...>   Quiddity.Identity.from_json(%{"root" => "text"})
      iex> message
      ~s(unknown kind "text")
  """
  @spec from_json(String.t() | map) :: {:ok, t} | {:error, InvalidIdentity.t()}
  def from_json(text) when is_binary(text) do
    case Quiddity.JSON.decode(text) do
      {:ok, doc} -> from_json(doc)
      {:error, message} -> {:error, InvalidIdentity.at("$", message)}
    end
  end

  def from_json(%{} = doc), do: catching(fn -> from_json!(doc) end)

  @doc "Reads an identity, as `from_json/1`, raising `Quiddity.InvalidIdentity`."
  @spec from_json!(String.t() | map) :: t
  def from_json!(text) when is_binary(text) do
    case from_json(text) do
      {:ok, id} -> id
      {:error, e} -> raise e
    end
  end

  def from_json!(%{} = doc) do
    {domain, root, defs} = Load.load!(doc)
    %__MODULE__{domain: domain, root: root, definitions: defs}
  end

  @doc """
  The identity's JSON form, in canonical form, as a map ready for
  `JSON.encode!/1`.
  """
  @spec to_json(t) :: map
  def to_json(%__MODULE__{} = id) do
    doc = %{"root" => id.root}
    doc = if id.domain, do: Map.put(doc, "domain", id.domain), else: doc
    if id.definitions == %{}, do: doc, else: Map.put(doc, "definitions", id.definitions)
  end

  # --- Digests ----------------------------------------------------------------

  @doc false
  def digest!(%__MODULE__{} = id, value) do
    source = Value.from(value)
    result = Apply.apply!(id.root, id.definitions, source)
    digest = Quiddity.hash(result)
    if id.domain, do: Quiddity.Digest.in_domain(digest, id.domain), else: digest
  end

  @doc false
  def value!(%__MODULE__{} = id, value) do
    Apply.apply!(id.root, id.definitions, Value.from(value))
  end

  @doc false
  def meta, do: :persistent_term.get({__MODULE__, :meta}, nil) || load_meta()

  defp load_meta do
    meta = from_json!(Meta.identity())
    :persistent_term.put({__MODULE__, :meta}, meta)
    meta
  end

  # --- Internals ----------------------------------------------------------------

  defp catching(fun) do
    {:ok, fun.()}
  rescue
    e in InvalidIdentity -> {:error, e}
  end

  defp invalid!(path, message), do: raise(InvalidIdentity.at(path, message))

  defp build!(root, defs, domain) do
    root = Load.canonical!(root, "$.root")
    defs = Map.new(defs, fn {n, k} -> {n, Load.canonical!(k, "$.definitions.#{n}")} end)

    if domain != nil and not (is_binary(domain) and domain != ""),
      do: invalid!("$.domain", "a domain is a non-empty string")

    {domain, root, defs} = Load.build!(root, defs, domain)
    %__MODULE__{domain: domain, root: root, definitions: defs}
  end

  # JSON numbers as spec 2.7 gives them: a float with an integer value is
  # that integer.
  # Kind terms as kinds in their JSON form; nested identities and recursive kinds stay
  # as {:identity, id} and :recursive until absorbed.
  @scalars ~w(null bool integer number string uuid path any)a

  defp to_doc(%__MODULE__{} = id, _path), do: {:identity, id}
  defp to_doc(:recursive, _path), do: :recursive
  defp to_doc(k, _path) when k in @scalars, do: Atom.to_string(k)
  defp to_doc(:bytes, _path), do: %{"bytes" => %{}}
  defp to_doc(:instant, _path), do: %{"instant" => %{}}
  defp to_doc(:duration, _path), do: %{"duration" => %{}}
  defp to_doc({:bytes, opts}, _path), do: %{"bytes" => stringify(opts)}
  defp to_doc({:instant, opts}, _path), do: %{"instant" => stringify(opts)}
  defp to_doc({:duration, opts}, _path), do: %{"duration" => stringify(opts)}
  defp to_doc({:list, k}, path), do: %{"list" => to_doc(k, path <> ".list")}
  defp to_doc({:set, k}, path), do: %{"set" => to_doc(k, path <> ".set")}
  defp to_doc({:optional, k}, path), do: %{"union" => ["null", to_doc(k, path)]}

  defp to_doc({:tuple, ks}, path) when is_list(ks),
    do: %{"tuple" => Enum.map(ks, &to_doc(&1, path))}

  defp to_doc({:union, ks}, path) when is_list(ks),
    do: %{"union" => Enum.map(ks, &to_doc(&1, path))}

  defp to_doc({:map, key, value}, path),
    do: %{"map" => %{"key" => to_doc(key, path), "value" => to_doc(value, path)}}

  defp to_doc({:object, fields}, path), do: to_doc({:object, fields, []}, path)

  defp to_doc({:object, fields, opts}, path) do
    names = Enum.map(fields, fn {n, _} -> to_string(n) end)

    if length(Enum.uniq(names)) != length(names),
      do: invalid!(path, "a field is given twice")

    doc = %{
      "fields" => Map.new(fields, fn {n, f} -> {to_string(n), field_doc(f, "#{path}.#{n}")} end)
    }

    doc = if Keyword.get(opts, :as_list), do: Map.put(doc, "order", names), else: doc
    %{"object" => doc}
  end

  defp to_doc({:tagged, variants}, path), do: to_doc({:tagged, variants, []}, path)

  defp to_doc({:tagged, variants, opts}, path) do
    doc = %{
      "variants" =>
        Map.new(variants, fn {n, v} -> {to_string(n), variant_doc(v, "#{path}.#{n}")} end)
    }

    doc =
      Enum.reduce(opts, doc, fn
        {:style, s}, acc -> Map.put(acc, "style", Atom.to_string(s))
        {:tag, t}, acc -> Map.put(acc, "tag", to_string(t))
        {:content, c}, acc -> Map.put(acc, "content", to_string(c))
        {:encode_as, e}, acc -> Map.put(acc, "encode_as", Atom.to_string(e))
        {k, _}, _acc -> invalid!(path, "unknown option #{inspect(k)} for a tagged union")
      end)

    %{"tagged" => doc}
  end

  defp to_doc(%{} = doc, _path), do: doc
  defp to_doc(k, _path) when is_binary(k), do: k
  defp to_doc(other, path), do: invalid!(path, "not a kind: #{inspect(other)}")

  defp stringify(opts),
    do:
      Map.new(opts, fn {k, v} ->
        {Atom.to_string(k), if(is_atom(v), do: Atom.to_string(v), else: v)}
      end)

  defp field_doc({:field, kind, opts}, path) do
    kind_doc = to_doc(kind, path)

    {kind_doc, omit} =
      cond do
        Keyword.get(opts, :omit_empty) -> {%{"union" => ["null", kind_doc]}, ["empty", "none"]}
        Keyword.get(opts, :optional) -> {%{"union" => ["null", kind_doc]}, ["none"]}
        true -> {kind_doc, []}
      end

    omit = Enum.uniq(omit ++ Enum.map(Keyword.get(opts, :omit, []), &Atom.to_string/1))
    doc = %{"kind" => kind_doc}
    doc = if omit != [], do: Map.put(doc, "omit", omit), else: doc

    Enum.reduce(Keyword.drop(opts, [:optional, :omit_empty, :omit]), doc, fn
      {:rename, r}, acc -> Map.put(acc, "rename", to_string(r))
      {:quantize, q}, acc -> Map.put(acc, "quantize", q)
      {:decimals, d}, acc -> Map.put(acc, "decimals", d)
      {k, _}, _acc -> invalid!(path, "unknown field option #{inspect(k)}")
    end)
  end

  defp field_doc(kind, path), do: %{"kind" => to_doc(kind, path)}

  defp variant_doc(nil, _path), do: %{}

  defp variant_doc({:variant, kind, opts}, path) do
    doc = if kind == nil, do: %{}, else: %{"kind" => to_doc(kind, path)}

    Enum.reduce(opts, doc, fn
      {:rename, r}, acc -> Map.put(acc, "rename", to_string(r))
      {:discriminant, d}, acc -> Map.put(acc, "discriminant", d)
      {k, _}, _acc -> invalid!(path, "unknown variant option #{inspect(k)}")
    end)
  end

  defp variant_doc(kind, path), do: %{"kind" => to_doc(kind, path)}

  # --- Composition: nested identities and recursion -------------------------------

  @self "_self"

  defp absorb_root(doc, defs) do
    if recursive?(doc) do
      defs = Map.put(defs, @self, "null")
      {kind, defs} = absorb(doc, defs, @self)
      {%{"ref" => @self}, Map.put(defs, @self, kind)}
    else
      absorb(doc, defs, nil)
    end
  end

  defp recursive?(:recursive), do: true
  defp recursive?({:identity, _}), do: false
  defp recursive?(%{} = m), do: Enum.any?(Map.values(m), &recursive?/1)
  defp recursive?(l) when is_list(l), do: Enum.any?(l, &recursive?/1)
  defp recursive?(_), do: false

  # The kind with nested identities moved into defs and replaced by
  # references, and recursive kinds by references to `me`.
  defp absorb({:identity, id}, defs, _me), do: merge(id, defs)

  defp absorb(:recursive, defs, nil),
    do: {invalid!("$", "a recursive kind outside an identity"), defs}

  defp absorb(:recursive, defs, me), do: {%{"ref" => me}, defs}

  defp absorb(%{} = m, defs, me) do
    Enum.reduce(m, {%{}, defs}, fn {k, v}, {acc, defs} ->
      {v, defs} = absorb(v, defs, me)
      {Map.put(acc, k, v), defs}
    end)
  end

  defp absorb(l, defs, me) when is_list(l) do
    {items, defs} =
      Enum.reduce(l, {[], defs}, fn x, {acc, defs} ->
        {x, defs} = absorb(x, defs, me)
        {[x | acc], defs}
      end)

    {Enum.reverse(items), defs}
  end

  defp absorb(x, defs, _me), do: {x, defs}

  # Moves a nested identity's definitions into defs, reusing equal ones and
  # renaming others whose names are taken; returns the kind standing for it.
  defp merge(%__MODULE__{} = id, defs) do
    {renames, _} =
      Enum.reduce(Enum.sort(id.definitions), {%{}, []}, fn {name, kind}, {renames, taken} ->
        target =
          case Map.fetch(defs, name) do
            :error -> name
            {:ok, ^kind} -> name
            {:ok, _} -> fresh(name, defs, taken)
          end

        {Map.put(renames, name, target), [target | taken]}
      end)

    defs =
      Enum.reduce(id.definitions, defs, fn {name, kind}, acc ->
        Map.put_new(acc, renames[name], rename_refs(kind, renames))
      end)

    {rename_refs(id.root, renames), defs}
  end

  defp fresh(base, defs, taken) do
    Stream.iterate(2, &(&1 + 1))
    |> Stream.map(&"#{base}_#{&1}")
    |> Enum.find(&(not Map.has_key?(defs, &1) and &1 not in taken))
  end

  defp rename_refs(%{"ref" => name}, renames), do: %{"ref" => Map.get(renames, name, name)}

  defp rename_refs(%{} = m, renames),
    do: Map.new(m, fn {k, v} -> {k, rename_refs(v, renames)} end)

  defp rename_refs(l, renames) when is_list(l), do: Enum.map(l, &rename_refs(&1, renames))
  defp rename_refs(x, _renames), do: x

  # Renames the definition `from` to `to`, and the references to it.
  defp rename_def(defs, from, to) do
    defs
    |> Map.new(fn {n, k} -> {if(n == from, do: to, else: n), rename_refs(k, %{from => to})} end)
  end

  # Makes `name` free in defs by renaming a definition that has it, except
  # the ones in `keep`.
  defp make_room(defs, name, keep) do
    if Map.has_key?(defs, name) and name not in keep,
      do: rename_def(defs, name, fresh(name, defs, [])),
      else: defs
  end

  # The object the root field functions change: the root, or the identity's
  # own definition. A recursive field makes the root its own definition.
  defp own_object!(%__MODULE__{root: %{"ref" => own}} = id, _field), do: {{:def, own}, id}

  defp own_object!(%__MODULE__{} = id, field) do
    if field != nil and recursive?(to_doc(field_kind(field), "$")) do
      defs = Map.put(id.definitions, @self, id.root)
      {{:def, @self}, %{id | root: %{"ref" => @self}, definitions: defs}}
    else
      {:root, id}
    end
  end

  defp field_kind({:field, kind, _}), do: kind
  defp field_kind(kind), do: kind

  defp edit_object!(id, target, fun) do
    object =
      case target do
        :root ->
          id.root

        {:def, name} ->
          Map.get(id.definitions, name) || invalid!("$", "no definition #{inspect(name)}")
      end

    case object do
      %{"object" => o} ->
        {fields, defs} = fun.(o["fields"], id.definitions)
        o = %{o | "fields" => fields}

        o =
          if Map.has_key?(o, "order"),
            do: %{
              o
              | "order" =>
                  Enum.filter(o["order"], &Map.has_key?(fields, &1)) ++
                    (Map.keys(fields) -- o["order"])
            },
            else: o

        case target do
          :root -> build!(%{"object" => o}, defs, id.domain)
          {:def, name} -> build!(id.root, Map.put(defs, name, %{"object" => o}), id.domain)
        end

      _ ->
        invalid!("$", "the identity's root is not an object")
    end
  end

  defp put_field(fields, defs, name, field, target) do
    me =
      case target do
        {:def, n} -> n
        :root -> nil
      end

    doc = field_doc(field, "$.#{name}")
    {doc, defs} = absorb(doc, defs, me)
    {Map.put(fields, to_string(name), doc), defs}
  end

  defimpl Quiddity.Encoder do
    def encode(identity, :primary) do
      meta = Quiddity.Identity.meta()
      Quiddity.Identity.value!(meta, Quiddity.Identity.to_json(identity))
    end

    def encode(_identity, name),
      do: raise(Quiddity.Error.at("$", "an identity has no identity #{inspect(name)}"))
  end

  # An identity's own digest is in the domain of identities (spec 5.10).
  @doc false
  def __quiddity__({:domain, :primary}), do: "quiddity.identity"
  def __quiddity__({:domain, _}), do: nil

  defimpl JSON.Encoder do
    def encode(identity, encoder), do: encoder.(Quiddity.Identity.to_json(identity), encoder)
  end

  if Code.ensure_loaded?(Jason.Encoder) do
    defimpl Jason.Encoder do
      def encode(identity, opts), do: Jason.Encode.map(Quiddity.Identity.to_json(identity), opts)
    end
  end
end
