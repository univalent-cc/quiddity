defmodule Quiddity.Typespec do
  @moduledoc false
  # A struct's field kinds, read from its `@type t` typespec, for exporting
  # its identity. Typespecs are read from the compiled module's debug
  # information (Code.Typespec.fetch_types/1); a module without one, or
  # without `t`, has no kinds, and its fields export as `:any`. A type this
  # module does not recognize is `:any` too: exporting never fails for
  # want of a typespec.

  @doc "The kinds of a struct's fields, by name; `%{}` when there is no usable typespec."
  @spec field_kinds(module) :: %{atom => term}
  def field_kinds(module) do
    with {:ok, types} <- Code.Typespec.fetch_types(module),
         locals = locals(types),
         {:ok, fields} <- struct_fields(locals, module) do
      Map.new(fields, fn {name, type} -> {name, kind(type, locals, 0)} end)
    else
      _ -> %{}
    end
  rescue
    _ -> %{}
  end

  defp locals(types) do
    for {kind, t} <- types, kind in [:type, :opaque], into: %{} do
      {:"::", _, [{name, _, args}, body]} = Code.Typespec.type_to_quoted(t)
      {{name, length(args || [])}, body}
    end
  end

  defp struct_fields(locals, module) do
    case Map.get(locals, {:t, 0}) do
      {:%, _, [^module, {:%{}, _, fields}]} -> {:ok, fields}
      _ -> :error
    end
  end

  # Deep enough for nested local types; a cycle of local types is `:any`.
  @max_depth 16

  defp kind(_type, _locals, depth) when depth > @max_depth, do: :any

  defp kind(nil, _locals, _depth), do: :null
  defp kind(atom, _locals, _depth) when is_atom(atom) and atom not in [true, false], do: :string
  defp kind(b, _locals, _depth) when is_boolean(b), do: :bool
  defp kind(n, _locals, _depth) when is_integer(n), do: :integer

  defp kind({:|, _, _} = union, locals, depth) do
    alternatives =
      union |> flatten_union() |> Enum.map(&kind(&1, locals, depth + 1)) |> Enum.uniq()

    case alternatives do
      [one] -> one
      many -> if :any in many, do: :any, else: union_kind(many)
    end
  end

  # [t], [t, ...]
  defp kind([item], locals, depth), do: {:list, kind(item, locals, depth + 1)}
  defp kind([item, {:..., _, _}], locals, depth), do: {:list, kind(item, locals, depth + 1)}
  defp kind([], _locals, _depth), do: {:list, :any}

  defp kind({:%{}, _, []}, _locals, _depth), do: {:map, :string, :any}

  defp kind({:%{}, _, [{{key_req, _, [key]}, value}]}, locals, depth)
       when key_req in [:optional, :required] do
    case kind(key, locals, depth + 1) do
      k when k in [:string, :integer] -> {:map, k, kind(value, locals, depth + 1)}
      _ -> :any
    end
  end

  # A struct literal of another struct: its exported identity.
  defp kind({:%, _, [module, _fields]}, _locals, _depth) when is_atom(module), do: nested(module)

  # Remote types: Module.t(...)
  defp kind({{:., _, [module, name]}, _, args}, locals, depth),
    do: remote(module, name, args, locals, depth)

  # Local types and built-in types: name(...)
  defp kind({name, _, args}, locals, depth) when is_atom(name) and is_list(args) do
    case Map.fetch(locals, {name, length(args)}) do
      {:ok, body} when args == [] -> kind(body, locals, depth + 1)
      _ -> builtin(name, args, locals, depth)
    end
  end

  defp kind({name, _, nil}, locals, depth) when is_atom(name),
    do: builtin(name, [], locals, depth)

  # Ranges of integers: 1..10
  defp kind({:.., _, [a, b]}, _locals, _depth) when is_integer(a) and is_integer(b), do: :integer

  defp kind(_type, _locals, _depth), do: :any

  @integers ~w(integer non_neg_integer pos_integer neg_integer byte char arity timeout)a
  @strings ~w(binary bitstring nonempty_binary nonempty_bitstring atom module node)a

  defp builtin(name, [], _locals, _depth) when name in @integers, do: :integer
  defp builtin(name, [], _locals, _depth) when name in [:float, :number], do: :number
  defp builtin(name, [], _locals, _depth) when name in @strings, do: :string
  defp builtin(:boolean, [], _locals, _depth), do: :bool
  defp builtin(nil, [], _locals, _depth), do: :null

  defp builtin(name, [item], locals, depth) when name in [:list, :nonempty_list],
    do: {:list, kind(item, locals, depth + 1)}

  defp builtin(name, [], _locals, _depth) when name in [:list, :nonempty_list], do: {:list, :any}
  defp builtin(:map, [], _locals, _depth), do: :any
  defp builtin(_name, _args, _locals, _depth), do: :any

  defp remote(String, :t, [], _locals, _depth), do: :string

  defp remote(module, :t, [], _locals, _depth) when module in [DateTime, NaiveDateTime],
    do: :instant

  defp remote(module, :t, [], _locals, _depth) when module in [Duration, Time],
    do: :duration

  defp remote(module, :t, [], _locals, _depth)
       when module in [Date, Decimal],
       do: :string

  defp remote(MapSet, :t, [item], locals, depth), do: {:set, kind(item, locals, depth + 1)}
  defp remote(MapSet, :t, [], _locals, _depth), do: {:set, :any}
  defp remote(Quiddity.Digest, :t, [], _locals, _depth), do: :string
  defp remote(module, :t, [], _locals, _depth), do: nested(module)
  defp remote(_module, _name, _args, _locals, _depth), do: :any

  # Another struct that knows its identity (a derived struct, an Ash
  # resource) exports as that identity; anything else as :any.
  defp nested(module) do
    exporting = Process.get(:quiddity_exporting, [])

    cond do
      # The struct being exported refers to itself: a recursive kind.
      match?([^module | _], exporting) -> :recursive
      # A longer cycle of structs: not followed.
      module in exporting -> :any
      true -> nested_identity(module)
    end
  end

  defp nested_identity(module) do
    if Code.ensure_loaded?(module) and function_exported?(module, :__quiddity__, 1) do
      case Quiddity.identity(module) do
        {:ok, identity} -> identity
        {:error, _} -> :any
      end
    else
      :any
    end
  end

  defp flatten_union({:|, _, [a, b]}), do: flatten_union(a) ++ flatten_union(b)
  defp flatten_union(t), do: [t]

  # A union of kinds, if it is a valid one (spec 5.7); `:any` otherwise. The
  # usual case is `t | nil`.
  defp union_kind(kinds) do
    {nulls, rest} = Enum.split_with(kinds, &(&1 == :null))

    kind =
      case rest do
        [one] -> one
        many -> {:union, many}
      end

    kind = if nulls != [], do: {:optional, kind}, else: kind

    # Checked on its own, where a recursive kind has no identity to refer
    # to: an object stands in for it, as it stands for an object.
    placeholder = replace_recursive(kind)

    case Quiddity.Identity.new(placeholder) do
      {:ok, _} -> kind
      {:error, _} -> :any
    end
  end

  defp replace_recursive(:recursive), do: {:object, []}
  defp replace_recursive({:optional, k}), do: {:optional, replace_recursive(k)}
  defp replace_recursive({:union, ks}), do: {:union, Enum.map(ks, &replace_recursive/1)}
  defp replace_recursive(k), do: k
end
