# Troubleshooting

This page covers common load test problems and their likely causes and fixes, helping you diagnose issues quickly.

If you are new to CreoBench, start with [Concepts](concepts.md) for an overview of the tool.

## Table of Contents

- [Dropped Transactions](#dropped-transactions)
- [Unexpected Request Timeouts](#unexpected-request-timeouts)
- [Connection Refused](#connection-refused)

---

## Dropped Transactions

A **dropped** transaction is one whose request could not be sent within 10 ms of its scheduled arrival, or whose script
failed before the request was sent. Any drop in the load phase means the load generators did not deliver the scheduled
load, so the run is invalid. At the end of a run, CreoBench logs a summary of all drops by reason. Dropped transactions
are also reported in the console output and the CSV files; see [Outputs](outputs.md) for details. The reason tells you
what to fix:

- **`NoVirtualUser`**: no virtual user became free in time, so the VU pool is too small for the load. Increase
  `--virtual-user-count` (see [Best practices](best-practices.md#virtual-user-count-sizing) for sizing), or check
  whether the system under test responds more slowly than expected. Long-running Lua code also keeps virtual users busy;
  review dynamic specification and extraction functions for expensive work.
- **`Late`**: a virtual user was free, but the load generator only got to the transaction too late, so it lacks CPU.
  Run it on a machine with more cores, or split the load across more load generator instances.
- **`Error(...)`**: the script failed before the request was sent. Check the error message for configuration problems,
  such as services referenced by the Lua script but missing from the [service registry](service-registry.md).

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
