# Troubleshooting

This page covers common load test problems and their likely causes and fixes, helping you diagnose issues quickly.

If you are new to CreoBench, start with [Concepts](concepts.md) for an overview of the tool.

## Table of Contents

- [Dropped Transactions](#dropped-transactions)
- [Unexpected Request Timeouts](#unexpected-request-timeouts)
- [Connection Refused](#connection-refused)

---

## Dropped Transactions

A **dropped** transaction is created but cannot be executed within the configured timeout. This commonly happens when
no virtual user is available to execute it, or when the load generator cannot keep up with the requested load. Dropped
transactions are reported in the console output and interval CSV; see [Outputs](outputs.md) for details.

Long-running Lua code can also keep virtual users busy and reduce the generator’s capacity. Review dynamic
specification and extraction functions, as well as any other script logic that runs before or after a request is sent.

To reduce dropped transactions, increase the virtual user count with `--virtual-user-count` if the VU pool is exhausted,
or reduce the load if the generator is at capacity. If the script is keeping VUs busy, simplify expensive computations
and avoid unnecessary work. Also check any error messages for configuration problems, such as services referenced by the
Lua script but missing from the [service registry](service-registry.md).

See [Scripting](scripting.md) for the virtual user behavior scripting API, [Load profiles](load-profiles.md) for
adjusting the load, and [Best practices](best-practices.md) for details on VU pool sizing.

---

## Unexpected Request Timeouts

A **timeout** means the request could not complete within the configured [timeout](cli-reference.md). This can happen
when the service is slow, unavailable, or when a connection cannot be established.

If the service normally takes longer to respond than the configured timeout, requests will time out even when the
service is working as expected. Delays during DNS resolution, TCP connection setup, or TLS handshakes can also
contribute to a timeout.

To troubleshoot, use `curl` or another HTTP client to verify that the URL in the [service registry](service-registry.md)
is reachable from the load generator. Check that the configured host can be resolved and that the target port accepts
connections. If the service is responding normally, increase `--timeout` to accommodate the expected response time of
the endpoints under test. Also verify that the TLS certificate and settings are compatible with the load generator.

---

## Connection Refused

A **connection refused** error means the orchestrator cannot establish a TCP connection to a load generator instance.
The load generator may not be running, may be listening on a different port, or the connection may be blocked by a
firewall.

Start `creo-load` on each configured machine before starting the orchestrator, and confirm that it is listening. Then
verify that the load generator’s `--listen-port` matches the port in the corresponding `--generator` address. The
default port is `24266`; custom ports must be specified as `--generator host:port`.

Finally, check the firewall rules on the load generator, the orchestrator, and any network between them. The
load generator port must allow inbound TCP connections from the orchestrator.

See the [CLI reference](cli-reference.md) for the available flags and
[Generator Address Format](cli-reference.md#generator-address-format) for accepted address formats.
