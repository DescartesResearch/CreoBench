# CLI Reference

This page lists every command-line flag for `creo-orch` and `creo-load`.
If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how the two commands work together.

## Table of Contents

- [creo-orch](#creo-orch)
- [creo-load](#creo-load)
- [Generator Address Format](#generator-address-format)

---

## creo-orch

The orchestrator coordinates the load test. It connects to one or more load
generator instances, distributes the test configuration, collects results,
and produces the final output.

```
creo-orch [OPTIONS] --generator <ADDRESS>...
```

| Name                   | Short | Type   | Required | Default | Description                                                                                                                        |
| ---------------------- | ----- | ------ | -------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `--generator`          | `-g`  | string | Yes      | —       | Address of a load generator instance. Repeat to add multiple instances. See [Generator Address Format](#generator-address-format). |
| `--profile`            | `-p`  | path   | No       | —       | Path to the [load profile](load-profiles.md) CSV file.                                                                             |
| `--script`             | `-l`  | path   | No       | —       | Path to the Lua [script](scripting.md) executed by each virtual user.                                                              |
| `--registry`           | `-r`  | path   | No       | —       | Path to the [service registry](service-registry.md) YAML file.                                                                     |
| `--warmup`             | `-w`  | path   | No       | —       | Path to the [warmup](warmup.md) YAML file. If not provided, the orchestrator looks for `warmup.yaml` in the current directory.     |
| `--output`             | `-o`  | path   | No       | —       | Directory where output files are written.                                                                                          |
| `--virtual-user-count` | `-u`  | u32    | No       | —       | Number of virtual users per load generator instance. Must be at least 1.                                                           |
| `--timeout`            | `-t`  | u64    | No       | —       | Transaction timeout in milliseconds.                                                                                               |
| `--seed`               | `-s`  | u64    | No       | —       | Seed for the random number generator.                                                                                              |
| `--overwrite-outputs`  | —     | flag   | No       | off     | Overwrite existing output files instead of aborting when the output directory is not empty.                                        |

---

## creo-load

A load generator instance receives instructions from the orchestrator and
sends transactions to the services under test.

```
creo-load [OPTIONS]
```

| Name            | Type | Required | Default | Description                                                          |
| --------------- | ---- | -------- | ------- | -------------------------------------------------------------------- |
| `--listen-port` | u16  | No       | `24266` | TCP port the load generator listens on for orchestrator connections. |

---

## Generator Address Format

The `--generator` flag accepts addresses in several formats. If the port is
omitted, the default port `24266` is used.

| Format        | Example              | Description                                      |
| ------------- | -------------------- | ------------------------------------------------ |
| `host:port`   | `10.0.0.1:8080`      | IPv4 address or hostname with an explicit port.  |
| `host`        | `my-generator.local` | IPv4 address or hostname using the default port. |
| `[IPv6]:port` | `[::1]:8888`         | IPv6 address in brackets with an explicit port.  |
| `[IPv6]`      | `[::1]`              | IPv6 address in brackets using the default port. |
