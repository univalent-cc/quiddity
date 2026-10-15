import Config

# For the test resources and the example scripts; an application that uses
# ash_quiddity sets these itself.
if config_env() in [:dev, :test] do
  config :ash, default_string_length_count: :codepoints

  config :ash,
    validate_domain_resource_inclusion?: false,
    validate_domain_config_inclusion?: false
end

if config_env() == :test do
  config :logger, level: :warning
end
