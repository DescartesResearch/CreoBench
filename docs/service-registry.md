# Service Registry

A service registry maps logical service names to base URLs. Lua scripts
refer to services by name, such as `auth` or `api`, and the registry
resolves those names to their actual endpoints at request time.

This lets you run the same script against different environments by changing
the registry file instead of modifying the script.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how service registries fit into a load test.

## Table of Contents

- [Registry Format](#registry-format)
- [URL Resolution](#url-resolution)
- [Validation Rules](#validation-rules)
- [Swapping Registries Across Environments](#swapping-registries-across-environments)

---

## Registry Format

A service registry is a YAML file with a flat mapping of string keys to
string values:

| Part      | Type   | Description                                                 |
| --------- | ------ | ----------------------------------------------------------- |
| **Key**   | string | Logical service name (e.g. `api`, `auth`).                  |
| **Value** | string | Base URL for that service (e.g. `https://api.example.com`). |

Each key is a name that Lua scripts use in the `service` field of a
request definition. Each value is the base URL that CreoBench sends
requests to. The script's `path` and `query` are appended to this URL
when a request is sent.

```yaml
# Each line maps a service name to its base URL.
api: https://api.example.com
auth: https://auth.example.com
```

---

## URL Resolution

When a virtual user executes an HTTP request, CreoBench resolves the URL
as follows:

1. Looks up the service name in the registry.
2. Parses the base URL.
3. Appends any `query` parameters to the base URL.
4. Sets the request's `path` on the resulting URL.

For example, given the registry:

```yaml
api: https://api.example.com
```

And a script request:

```lua
{
    method = "GET",
    service = "api",
    path = "/users/42",
}
```

CreoBench sends a GET request to `https://api.example.com/users/42`.

If the service name is not in the registry, the transaction fails with
an error indicating the service was not found.

---

## Validation Rules

The orchestrator validates the registry file before starting a load test.
If the validation fails, the load test will not begin.

**The registry must not be empty.** At least one service mapping is
required.

If a base URL is not valid, the error surfaces when the virtual user
sends a request to that service, not at load-test startup.

---

## Swapping Registries Across Environments

The main benefit of a service registry is that the same Lua script works
across environments. You can create a separate registry file for each
environment and point the orchestrator to the corresponding file.

### Development (local)

```yaml
# registry-dev.yaml
auth: http://localhost:4000
api: http://localhost:3000
users: http://localhost:5000
```

### Private Cloud

```yaml
# registry-private-cloud.yaml
auth: http://10.10.10.1
api: http://10.10.10.2
users: http://10.10.10.3
```

### Public Cloud

```yaml
# registry-public-cloud.yaml
auth: https://auth.example.com
api: https://api.example.com
users: https://users.example.com
```

Run the same test against any environment by passing the corresponding
file:

```bash
# Against local
creo-orch -r registry-dev.yaml -g localhost:8080 -l my-test.lua

# Against private cloud
creo-orch -r registry-private-cloud.yaml -g localhost:8080 -l my-test.lua

# Against public cloud
creo-orch -r registry-public-cloud.yaml -g localhost:8080 -l my-test.lua
```

No changes to the Lua script are required.
