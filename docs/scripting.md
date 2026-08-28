# Virtual Users

A load test's Lua script defines the behavior that all virtual users execute,
though each VU may be at a different point in the request sequence. Each
VU runs this Lua script while maintaining its own isolated state, including
cookies, session data, and a per-VU key-value store, allowing the same
behavioral template to scale across hundreds or thousands of concurrent sessions.

This page documents the full scripting API. The scripting API gives you full control
over request sequencing, data handling, and state management to accurately simulate
real-world user patterns.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how virtual users fit into a load test.

## Table of Contents

- [Script Structure](#script-structure)
- [Request Specs](#request-specs)
  - [Static Specs](#static-specs)
  - [Dynamic Specs](#dynamic-specs)
- [HTTP Spec Fields](#http-spec-fields)
  - [HTTP Methods](#http-methods)
- [The Jump Protocol](#the-jump-protocol)
- [The Store](#the-store)
- [Extract Functions](#extract-functions)
- [Response API](#response-api)
- [Cookie Handling](#cookie-handling)
- [Putting It All Together](#putting-it-all-together)

---

## Script Structure

Define virtual user behavior using a Lua script that returns a table with two functions:

```lua
local function setup(store)
    -- One-time requests run by each VU before the load phase begins.
    return {}
end

local function requests(store)
    -- The user loop: cycled continuously during the load phase.
    return {
        {
            protocol = "http",
            method = "GET",
            service = "api",
            path = "/",
        },
    }
end

return { setup = setup, requests = requests }
```

Both functions receive a [store](#the-store) as their first argument, which
can be used to store state for the lifecycle of the user.
Both return an array of [request specs](#request-specs) -- specifications
that define individual requests. Each spec is either a
[static](#static-specs) table (a fixed request definition) or a
function (a [dynamic](#dynamic-specs) spec that generates a request specification at
runtime). See the official Lua documentation about [tables](https://www.lua.org/pil/2.5.html)
and [arrays](https://www.lua.org/pil/11.1.html) if you're unfamiliar with Lua's array syntax.

**Setup** executes once per VU before the load phase begins. Use it to authenticate,
acquire session tokens, or create test data. Setup specs run in declaration order
and complete before the user loop starts. If setup returns an empty array, no setup
requests are sent.

**Requests** is the user loop. After the last spec is executed, the loop wraps back
to the first spec and continues cycling for the duration of the load phase. The
`requests` function must return at least one spec.

---

## Request Specs

Each entry returned by `setup` or `requests` is a **request spec**. There
are two kinds: static and dynamic.

### Static Specs

A static spec is a Lua table describing an HTTP request. It is parsed once
at script load time.

```lua
-- Static Spec
{
    protocol = "http",
    method = "POST",
    service = "auth",
    path = "/login",
    body = { username = "demo", password = "secret" },
}
```

The protocol field defines the protocol used for the request. Currently,
only HTTP is supported. Additional protocols, such as gRPC, may be supported
in the future.

### Dynamic Specs

A dynamic spec is a Lua function that is called each time the spec is executed.
It receives the [store](#the-store) and must return either:

- A **table** — parsed as a static spec.
- A **string** — the name of a named spec to jump to (see
  [The Jump Protocol](#the-jump-protocol)).

```lua
-- Dynamic Spec
function(store)
    local token = store:get("token")
    return {
        protocol = "http",
        method = "GET",
        service = "api",
        path = "/protected",
        headers = {
            Authorization = "Bearer " .. token,
        },
    }
end
```

Dynamic specs let a request depend on prior responses or shared state
per execution.

---

## HTTP Spec Fields

Every HTTP request spec has the following fields:

| Field      | Required | Type                                     | Description                                                                                                                                    |
| ---------- | -------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `protocol` | Yes      | string                                   | Must be `"http"`.                                                                                                                              |
| `method`   | Yes      | string                                   | The HTTP method (see [HTTP Methods](#http-methods)).                                                                                           |
| `service`  | Yes      | string                                   | Logical service name, resolved by the [service registry](service-registry.md).                                                                 |
| `path`     | Yes      | string                                   | URL path appended to the service's base URL.                                                                                                   |
| `headers`  | No       | table                                    | Header key-value pairs. Values can be strings, numbers, or booleans (coerced to strings).                                                      |
| `query`    | No       | table                                    | Query parameter key-value pairs. Values are coerced to strings using the same rules as headers. Query parameters are automatically URL-encoded |
| `body`     | No       | string, number, boolean, table, or array | Request body, serialized as JSON.                                                                                                              |
| `extract`  | No       | function                                 | Callback invoked after a successful response (see [Extract Functions](#extract-functions)).                                                    |

If `headers` or `query` is omitted or set to `nil`, no headers or query
parameters are sent. Note that cookies are automatically sent (see
[Cookie Handling](#cookie-handling)). If `body` is omitted, no request body is sent.

---

### HTTP Methods

CreoBench supports seven standard HTTP methods. The `method` field is
case-insensitive:

| Method    | Example                    |
| --------- | -------------------------- |
| `GET`     | `"GET"` or `"get"`         |
| `POST`    | `"POST"` or `"post"`       |
| `PUT`     | `"PUT"` or `"put"`         |
| `DELETE`  | `"DELETE"` or `"delete"`   |
| `PATCH`   | `"PATCH"` or `"patch"`     |
| `HEAD`    | `"HEAD"` or `"head"`       |
| `OPTIONS` | `"OPTIONS"` or `"options"` |

Any other method value causes a script load error.

---

## The Jump Protocol

So far we've only looked at anonymous specs—specs without a name. You can also give
a static spec a `name` to support conditional branching through the **jump protocol**.

A named spec is a table with two keys: `name` (a string) and `spec` (either a static
or dynamic spec):

```lua
-- Named static spec
{
    name = "deletePost",
    spec = {
        protocol = "http",
        method = "DELETE",
        service = "posts",
        path = "/posts/1",
    },
}
```

The jump protocol lets a dynamic spec redirect execution to a named spec
anywhere in the spec list. This is useful when the next request
depends on the outcome of a prior response. For example, skipping a
create step if the resource already exists.

To define a jump, a dynamic spec can return a **string** instead of a static spec.
The string must match the `name` of a named spec in the spec list. The VU
resolves the jump and executes the target spec.

```lua
local function requests()
    return {
        {
            name = "createPost",
            spec = {
                protocol = "http",
                method = "POST",
                service = "posts",
                path = "/posts",
                body = { title = "Hello" },
            },
        },
        --- ...
        function(store)
            -- Jump back to "createPost" if shouldRetry is set
            -- Imagine this would be set by a prior spec (not shown in this example).
            if store:get("shouldRetry") then
                return "createPost"
            end
            -- Otherwise, return a concrete spec.
            return {
                protocol = "http",
                method = "DELETE",
                service = "posts",
                path = "/posts/1",
            }
        end,
    }
end

return { setup = setup, requests = requests }
```

Jump resolution works as follows:

1. The dynamic function is called.
2. If it returns a string, the virtual user jumps to the spec with this name.
3. If the jump target is a static spec, it is executed.
4. If the jump target is a dynamic spec, its function is called. If it returns
   a static spec it is executed. If it returns a string, the jump is resolved starting
   from step 2.

### Caveats

Multiple jumps in a single transaction may delay the time until a request is actually sent,
since the virtual user must resolve all jumps until a static request spec is found. CreoBench
currently does not detect infinite loops in a jump sequence, so take care to avoid circular
references. If a jump targets a name that does not exist in the spec list, the transaction fails
with an error.

---

## The Store

The **Store** is a per-VU key-value store for sharing state across requests.
It is passed as the first argument to `setup(store)`, `requests(store)`,
[dynamic spec](#dynamic-specs) functions, and [extract](#extract-functions) functions.
The store provides the following methods:

| Method                  | Arguments                 | Returns        | Description                                               |
| ----------------------- | ------------------------- | -------------- | --------------------------------------------------------- |
| `store:get(key)`        | key (string)              | value or `nil` | Returns the value stored under `key`, or `nil` if absent. |
| `store:set(key, value)` | key (string), value (any) | nothing        | Stores `value` under `key`, overwriting any prior value.  |

The store supports all Lua value types: strings, numbers, booleans, `nil`,
tables (arrays and objects), and functions. Each VU has its own isolated
store — VUs do not share data.

```lua
local function setup(store)
    store:set("page", math.random(1, 10))
    return {}
end

local function requests(store)
    local page = store:get("page")

    return {
        {
            protocol = "http",
            method = "GET",
            service = "api",
            path = "/items",
            query = { page = page },
        },
    }
end

return { setup = setup, requests = requests }
```

---

## Extract Functions

An **extract** function is an optional callback on a request spec. It runs
after a successful HTTP response (status code below 400) and receives two
arguments: the [store](#the-store) and the [response](#response-api).

```lua
{
    protocol = "http",
    service = "auth",
    method = "POST",
    path = "/login",
    body = { username = "demo", password = "secret" },
    extract = function(store, response)
        local data, err = response:json()
        if not data then
            error("login failed: " .. tostring(err))
        end
        store:set("token", data.accessToken)
    end,
}
```

The extract function can read the response body, headers, or status code (see
[Response API](#response-api)) and write values into the store. Subsequent requests can
read those values with `store:get(...)`. This is the primary mechanism for carrying
over state, e.g, a login response provides a token, which a subsequent request reads
from the store to include in an `Authorization` header.

---

## Response API

A **response** object is passed to extract functions as the **second** argument. It provides three
methods for inspecting the HTTP response:

| Method                  | Returns           | Notes                                                                                                           |
| ----------------------- | ----------------- | --------------------------------------------------------------------------------------------------------------- |
| `response:status()`     | number            | The HTTP status code (e.g. `200`, `404`).                                                                       |
| `response:header(name)` | string or `nil`   | Case-insensitive lookup. Returns `nil` if absent. Multiple values are joined with `", "`.                       |
| `response:json()`       | table, nil, error | Returns `(data, nil)` on success, `(nil, error_string)` on parse failure, or `(nil, nil)` if the body is empty. |

```lua
extract = function(store, response)
    local status = response:status()
    local ct = response:header("Content-Type")
    local data, err = response:json()

    store:set("status", status)
    store:set("contentType", ct)

    if data then
        store:set("userId", data.id)
    end
end
```

---

## Cookie Handling

Each virtual user automatically stores **cookies**. The cookies are tracked
per-VU and are not shared across multiple users. After each HTTP response,
`Set-Cookie` headers are captured and stored. Domain, path, `Secure`,
`HttpOnly`, and `Max-Age` semantics are respected.

You do not need to manage cookies manually. If your application sets
session cookies during the setup phase, they will be automatically included
in subsequent requests.

---

## Putting It All Together

Here is a complete script that exercises the main features of the scripting
virtual user scripting API:

```lua
local function setup(store)
    return {
        {
            protocol = "http",
            method = "POST",
            service = "auth",
            path = "/login",
            body = { username = "demo", password = "secret" },
            extract = function(store, response)
                local data, err = response:json()
                if not data then
                    error("login failed: " .. tostring(err))
                end
                store:set("token", data.accessToken)
            end,
        },
    }
end

local function requests(store)
    return {
        {
            -- list all items
            protocol = "http",
            method = "GET",
            service = "api",
            path = "/items",
            headers = {
                Authorization = "Bearer " .. store:get("token"),
            },
        },
        {
            name = "createItem",
            spec = function(store)
            -- Create a new item, and store its ID in the store.
                return {
                    protocol = "http",
                    method = "POST",
                    service = "api",
                    path = "/items",
                    headers = {
                        Authorization = "Bearer " .. store:get("token"),
                        ["Content-Type"] = "application/json",
                    },
                    body = { title = "New item" },
                    extract = function(store, response)
                        local data, err = response:json()
                        if data then
                           store:set("lastItemId", data.id)
                        end
                    end,
                }
            end,
        },
        function(store)
            local id = store:get("lastItemId")
            -- Check if create item succeeded
            if not id then
                return "createItem"
            end

            return {
                protocol = "http",
                method = "GET",
                service = "api",
                path = "items" .. tostring(id),
                headers = {
                    Authorization = "Bearer " .. store:get("token"),
                },
            }
        end,
    }
end

return { setup = setup, requests = requests }
```

This script logs in during setup, stores the access token, then loops
through listing items, creating a new item, and fetching that item by ID.
Each request includes the token from the store, and the extract function
captures the ID of the newly created item. If the create step fails —
either because the response had a 4xx status code and the extract did not
run, or the response body could not be parsed — the fetch step jumps back
to the create step via the jump protocol to retry.
