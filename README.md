<h1 align="center">CreoBench</h1>

<p align="center">
  A distributed, open-loop load-testing tool written in Rust.
</p>

<p align="center">
  <a href="https://www.gnu.org/licenses/agpl-3.0">
    <img src="https://img.shields.io/badge/License-AGPLv3-blue.svg" alt="License: AGPL v3">
  </a>
  <a href="https://github.com/DescartesResearch/CreoBench/actions/workflows/checks.yml">
    <img src="https://github.com/DescartesResearch/CreoBench/actions/workflows/checks.yml/badge.svg" alt="checks">
  </a>
  <a href="https://github.com/DescartesResearch/CreoBench/actions/workflows/tests.yml">
    <img src="https://github.com/DescartesResearch/CreoBench/actions/workflows/tests.yml/badge.svg" alt="tests">
  </a>
  <a href="https://codecov.io/gh/DescartesResearch/CreoBench">
    <img src="https://codecov.io/gh/DescartesResearch/CreoBench/branch/main/graph/badge.svg" alt="code coverage" >
  </a>
</p>

CreoBench is a distributed, open-loop load-testing tool written in Rust. It lets you define virtual-user behavior in
Lua, generate time-varying workloads from multiple load generators, and collect both interval-level metrics and
per-transaction results for detailed analysis.

- **Distributed execution** — generate load from multiple workers while orchestrating the test from a single process.
- **Open-loop load generation** — generate requests from a specified arrival process, including exponentially
  distributed inter-arrival times that approximate independent user or event arrivals. Because generation is decoupled
  from response completion, system slowdowns do not reduce the offered load as they do in closed-loop tests, enabling
  controlled measurements of throughput, queueing, saturation, and tail latency.
- **Flexible Lua workloads** — model realistic user behavior in Lua, from simple request sequences to stateful users
  with branching logic, parameterization, and dynamically selected operations.
- **Arbitrary load profiles** — generate workloads ranging from constant request rates to complex time-varying profiles
  derived from experiments or production traces, using CSV input.
- **Configurable environments** — run the same Lua workload against different system configurations, such as local
  deployments, private clusters, or public clouds by only changing a YAML registry.
- **Interval metrics** — export per-interval success, failure, timeout, and dropped-request counts, along with
  response-time statistics, to CSV.
- **Per-transaction detail** — record every request with its response time, service time, status, and classification
  for flexible analysis.

## Table of Contents

- [Installing from source](#installing-from-source)
- [Documentation](#documentation)
- [Architecture](#architecture)
- [Quickstart](#quickstart)
  - [Load profile](#load-profile)
  - [Virtual-user script](#virtual-user-script)
  - [Service registry](#service-registry)
  - [Warmup config](#warmup-config)
  - [Running the load test](#running-the-load-test)
- [Outputs](#outputs)
  - [`interval.csv`](#intervalcsv)
  - [`transactions.csv`](#transactionscsv)
  - [Progress Output](#progress-output)
- [License](#license)

## Installing from source

CreoBench supports Rust version **1.92** (or later).

```sh
cargo install --path . --locked
```

This installs two binaries:

| Binary      | Purpose                                        |
| ----------- | ---------------------------------------------- |
| `creo-orch` | Coordinates the load test and collects results |
| `creo-load` | Executes the virtual user request loop         |

The next section, explains the components and architecture in more detail.

## Documentation

Full documentation, including guides and reference material, is available in
[`docs/`](docs/README.md). To get started quickly, see the [Quickstart](#quickstart) section below.

## Architecture

```
┌──────────────┐         TCP          ┌────────────────┐
│              │ ◄──────────────────► │                │
│ Orchestrator │ ◄──────────────────► │ Load Generator │
│              │ ◄──────────────────► │       ×N       │
└──────────────┘                      └────────────────┘
```

CreoBench separates test coordination from load generation. The orchestrator acts as the central controller:
it connects to the load generators, distributes the test configuration, coordinates execution, and collects their
results. The load generators execute the workload and produce requests against the system under test.

Communication between the orchestrator and each load generator uses TCP. Each generator operates independently,
allowing the workload to be distributed across multiple processes or machines while remaining controlled from a single
orchestration point.

A typical load test proceeds as follows:

1. Launch the required number of load generators.
2. Start the orchestrator, which connects to the configured generators.
3. The orchestrator distributes the test configuration and workload definition to each generator.
4. Each generator initializes its virtual users and completes any required setup.
5. Once all generators are ready, the orchestrator instructs them to start the load test simultaneously.
6. The generators issue requests according to the configured load profile and virtual-user behavior.
7. During the test, each generator records per-transaction results, aggregates interval metrics, and reports the
   results to the orchestrator.
8. The orchestrator collects and combines the results, then exports them for analysis.

For small experiments, the orchestrator and load generators can run on the same machine. For larger workloads,
generators can be placed on separate machines to increase the available request-generation capacity and distribute
network traffic. Adding generators increases the number of independent load sources and equally distributes the target
load across multiple machines.

## Quickstart

This quickstart runs a small, complete load test against `https://example.com`.
The test is driven by four files in `examples/`:

- `profile.csv` — the load-phase request profile
- `script.lua` — virtual-user behavior
- `registry.yaml` — logical service names and their base URLs
- `warmup.yaml` — the optional warmup phase

The example uses intentionally small values so that it completes quickly. A
real load test would typically run for several minutes or hours with a larger
virtual user pool and a longer load profile.

### Load profile

The load profile is defined in `examples/profile.csv`:

```csv
deadline,count
1.0,1
2.0,2
3.0,3
4.0,2
5.0,1
```

Each row specifies target time in seconds and the number of requests to generate until this target time.
The count is not cumulative: for example, this profile sends 1 request during the first second, 2 requests between
seconds 1 and 2, 3 requests between seconds 2 and 3, and so on. The example therefore ramps the request rate up for
the first three seconds before ramping it back down.

Real benchmarks can use longer profiles with any desired shape, including
constant rates, ramps, bursts, and profiles derived from production traces.
For example, a benchmark might ramp up gradually, sustain the target load for
several minutes, and then ramp down.

### Virtual-user script

Virtual-user behavior is defined in `examples/script.lua`:

```lua
local function setup()
	return {}
end

local function requests()
	return {
		{
			protocol = "http",
			method = "GET",
			service = "api",
			path = "/",
			headers = {
				Accept = "application/json",
			},
			query = {
				greeting = "world",
			},
		},
	}
end

return { setup = setup, requests = requests }
```

The script defines the actions performed by each virtual user:

- `setup()` runs once for each virtual user before the load phase. It can be used to perform initialization such as
  authentication or session acquisition.
- `requests()` returns the request specifications used by the virtual user. In this example, each user repeatedly sends
  a GET request to /. More complex scripts can use Lua state, branching, parameterization, and dynamic request
  selection to model different user behaviors.

### Service registry

The logical service used by the script is mapped to a concrete base URL in
`examples/registry.yaml`:

```yaml
# Logical service name -> base URL. The script's `service` field is
# resolved against this mapping when the request is sent.
api: https://example.com
```

The script refers to the service by its logical name, `api`, rather than
embedding a URL. To run the same workload against another deployment, change
the registry instead of modifying the Lua script.

### Warmup config

The optional warmup phase is configured in `examples/warmup.yaml`:

```yaml
# The warmup phase runs before the main load test and is meant to let
# the system under test stabilise. `pause` is the gap between warmup and load.
rate: 1 # transactions per second
duration: 5 # seconds
pause: 2 # seconds
```

Warmup allows the system to initialize caches, JIT compilers, connection pools,
and other components before measurements begin. The pause then provides a
short settling period between warmup and the main load phase.

This example runs a five-second warmup at one transaction per second, followed
by a two-second pause. For a load test intended to measure steady-state
performance, choose a warmup duration and rate representative of the target
workload, and allow enough time for the system to settle.

### Running the load test

**Step 1 -- start the load generator:**

```sh
creo-load
```

By default, the generator listens on port **24266** for connections from the
orchestrator.

**Step 2 -- start the orchestrator:**

```sh
creo-orch \
  --profile examples/profile.csv \
  --script examples/script.lua \
  --registry examples/registry.yaml \
  --warmup examples/warmup.yaml \
  --generator 127.0.0.1 \
  --virtual-user-count 10 \
  --output results
```

The orchestrator connects to the load generator, distributes the test
configuration, runs the warmup phase, executes the load profile, and writes
the resulting files to the `results/` directory.

Use `--help` on either binary for the full list of options.

## Outputs

By default, CreoBench writes two CSV files to `./results`. Use the
`--output` option to specify a different output directory.

### `interval.csv`

`interval.csv` contains one row for each load interval. Metrics are aggregated across all load generators per interval.

| Column                    | Description                                                                                                   |
| ------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `target_time`             | Deadline, relative to the start of the load test, for sending all transactions in this load interval          |
| `load_level`              | Configured target request rate for this interval                                                              |
| `successful_transactions` | Transactions that completed successfully during this interval                                                 |
| `failed_transactions`     | Transactions that failed during this interval                                                                 |
| `timeout_transactions`    | Transactions that exceeded the configured timeout during this interval                                        |
| `dropped_transactions`    | Transactions that were never sent (e.g. due to VU pool exhaustion, or Lua script errors) during this interval |
| `avg_service_time`        | Average service time in milliseconds of non-dropped transactions that finished during this interval           |
| `final_batch_time`        | Time at which the final request batch was sent in this interval                                               |
| `phase`                   | Indicates the phase of the load interval: `warmup`, `pause`, or `load`                                        |

The metrics in `interval.csv` group transactions by the interval in which they finished, not the interval in which they
were sent. Consequently, a transaction sent during one interval may be included in the metrics for a later interval if
it completes after the original interval has ended.

A dropped transaction is one that was scheduled or started by the load generator but never sent to the system under
test. Drops can occur when the generator cannot assign the transaction to an available virtual user, when workload
execution fails before the request is sent, or when a scheduling delay means that the transaction can no longer be sent
within its intended deadline. A high number of dropped transactions may indicate insufficient VU pool size or
load-generator capacity rather than a failure in the system under test.

### `transactions.csv`

`transactions.csv` contains one row for every transaction, including transactions that were dropped before reaching the
network. This can be used for detailed post-hoc analysis.

| Column              | Description                                                                                         |
| ------------------- | --------------------------------------------------------------------------------------------------- |
| `target_time`       | Deadline, relative to the start of the load test, for sending this transaction                      |
| `start_time`        | Time at which the transaction was queued, relative to the start of the load test                    |
| `load_generator_id` | Identifier of the load generator that sent the transaction                                          |
| `virtual_user_id`   | Identifier of the virtual user that sent the transaction                                            |
| `spec_id`           | Index of the request specification in the script's request table that was sent for this transaction |
| `response_time_ms`  | End-to-end time in milliseconds including wait time                                                 |
| `service_time_ms`   | Time in milliseconds the transaction waited for a server response                                   |
| `outcome`           | Transaction outcome: `success`, `failed`, `timeout`, or `dropped`                                   |
| `reason`            | Error classification for non-success outcomes; `null` for successful transactions                   |

### Progress Output

During execution, CreoBench also writes progress information to `stderr`.
It prints one summary line per load interval:

```
TARGET=8.0s; LOAD=100; #SUCC=2; #FAIL=1; #TO=0; #DROP=0; AVG ST=100ms
```

The line reports the load interval target time, configured load level, successful
and failed transaction counts, timeouts, dropped transactions, and average service time.

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE).
