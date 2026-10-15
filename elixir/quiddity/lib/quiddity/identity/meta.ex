defmodule Quiddity.Identity.Meta do
  @moduledoc false
  # The identity of identities (spec Appendix C), under which an
  # identity's own digest is taken (spec 5.10).

  @identity %{
    "domain" => "quiddity.identity",
    "root" => %{
      "ref" => "Identity"
    },
    "definitions" => %{
      "Identity" => %{
        "object" => %{
          "fields" => %{
            "domain" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "root" => %{
              "kind" => %{
                "ref" => "Kind"
              }
            },
            "definitions" => %{
              "kind" => %{
                "union" => [
                  "null",
                  %{
                    "map" => %{
                      "key" => "string",
                      "value" => %{
                        "ref" => "Kind"
                      }
                    }
                  }
                ]
              },
              "omit" => ["none", "empty"]
            }
          }
        }
      },
      "Kind" => %{
        "tagged" => %{
          "variants" => %{
            "null" => %{},
            "bool" => %{},
            "integer" => %{},
            "number" => %{},
            "string" => %{},
            "uuid" => %{},
            "path" => %{},
            "any" => %{},
            "bytes" => %{
              "kind" => %{
                "object" => %{
                  "fields" => %{
                    "text" => %{
                      "kind" => %{
                        "union" => ["null", "string"]
                      },
                      "omit" => ["none"]
                    }
                  }
                }
              }
            },
            "instant" => %{
              "kind" => %{
                "ref" => "Time"
              }
            },
            "duration" => %{
              "kind" => %{
                "ref" => "Time"
              }
            },
            "list" => %{
              "kind" => %{
                "ref" => "Kind"
              }
            },
            "set" => %{
              "kind" => %{
                "ref" => "Kind"
              }
            },
            "tuple" => %{
              "kind" => %{
                "list" => %{
                  "ref" => "Kind"
                }
              }
            },
            "map" => %{
              "kind" => %{
                "object" => %{
                  "fields" => %{
                    "key" => %{
                      "kind" => %{
                        "ref" => "Kind"
                      }
                    },
                    "value" => %{
                      "kind" => %{
                        "ref" => "Kind"
                      }
                    }
                  }
                }
              }
            },
            "object" => %{
              "kind" => %{
                "ref" => "Object"
              }
            },
            "union" => %{
              "kind" => %{
                "set" => %{
                  "ref" => "Kind"
                }
              }
            },
            "tagged" => %{
              "kind" => %{
                "ref" => "Tagged"
              }
            },
            "ref" => %{
              "kind" => "string"
            }
          }
        }
      },
      "Time" => %{
        "object" => %{
          "fields" => %{
            "unit" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            }
          }
        }
      },
      "Object" => %{
        "object" => %{
          "fields" => %{
            "fields" => %{
              "kind" => %{
                "map" => %{
                  "key" => "string",
                  "value" => %{
                    "ref" => "Field"
                  }
                }
              }
            },
            "unlisted" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "order" => %{
              "kind" => %{
                "union" => [
                  "null",
                  %{
                    "list" => "string"
                  }
                ]
              },
              "omit" => ["none"]
            }
          }
        }
      },
      "Field" => %{
        "object" => %{
          "fields" => %{
            "kind" => %{
              "kind" => %{
                "ref" => "Kind"
              }
            },
            "rename" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "omit" => %{
              "kind" => %{
                "union" => [
                  "null",
                  %{
                    "set" => "string"
                  }
                ]
              },
              "omit" => ["none", "empty"]
            },
            "quantize" => %{
              "kind" => %{
                "union" => ["null", "integer"]
              },
              "omit" => ["none"]
            },
            "decimals" => %{
              "kind" => %{
                "union" => ["null", "integer"]
              },
              "omit" => ["none"]
            }
          }
        }
      },
      "Tagged" => %{
        "object" => %{
          "fields" => %{
            "variants" => %{
              "kind" => %{
                "map" => %{
                  "key" => "string",
                  "value" => %{
                    "ref" => "Variant"
                  }
                }
              }
            },
            "style" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "tag" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "content" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "encode_as" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            }
          }
        }
      },
      "Variant" => %{
        "object" => %{
          "fields" => %{
            "kind" => %{
              "kind" => %{
                "union" => [
                  "null",
                  %{
                    "ref" => "Kind"
                  }
                ]
              },
              "omit" => ["none"]
            },
            "rename" => %{
              "kind" => %{
                "union" => ["null", "string"]
              },
              "omit" => ["none"]
            },
            "discriminant" => %{
              "kind" => %{
                "union" => ["null", "integer"]
              },
              "omit" => ["none"]
            }
          }
        }
      }
    }
  }

  def identity, do: @identity
end
