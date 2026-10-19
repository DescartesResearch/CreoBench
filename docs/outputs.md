# Outputs

CreoBench produces several outputs during and after a load test: a live
console progress line, an interval CSV summarizing each load interval, a
transaction CSV recording every individual transaction, and a record of the
run itself: copies of its input files and its metadata. This page documents
each output so you can analyze your results.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how the reporting system fits into a load test.

## Table of Contents

- [Console Output](#console-output)
- [Interval CSV](#interval-csv)
- [Transaction CSV](#transaction-csv)
- [Input Files](#input-files)
- [Run Metadata](#run-metadata)

---

## Console Output

CreoBench prints a live progress line to **stderr** after each load interval.
This gives you a quick view of the test's status while it runs.

The format is:

```
TARGET=1.0s; LOAD=10; #SUCC=5; #FAIL=3; #TO=1; #DROP=1; AVG ST=134ms
```

Each field is described below.

| Field    | Description                                             |
| -------- | ------------------------------------------------------- |
| `TARGET` | The target time of the load interval.                   |
| `LOAD`   | The number of transactions scheduled for this interval. |
| `#SUCC`  | Count of successful transactions.                       |
| `#FAIL`  | Count of failed transactions.                           |
| `#TO`    | Count of timed-out transactions.                        |
| `#DROP`  | Count of dropped transactions.                          |
| `AVG ST` | Average service time of transactions in milliseconds.   |

---

## Interval CSV

The interval CSV provides one row per load interval. It gives you an
aggregated view of how the load test performed over time, including counts
for each outcome type and the average service time. It contains the same
data as the console output.

| Column                    | Type   | Description                                                                                                                                                       |
| ------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `target_time`             | `f64`  | The target time of the load interval, as seconds since the load test **phase** started.                                                                           |
| `load_level`              | `u32`  | The target number of transactions for this interval. Arrivals are random, so the actual number varies around it.                                                  |
| `successful_transactions` | `u64`  | Number of transactions that completed successfully.                                                                                                               |
| `failed_transactions`     | `u64`  | Number of transactions that failed.                                                                                                                               |
| `timeout_transactions`    | `u64`  | Number of transactions that timed out.                                                                                                                            |
| `dropped_transactions`    | `u64`  | Number of transactions that were dropped.                                                                                                                         |
| `avg_service_time`        | `u64`  | Average service time of non-dropped transactions.                                                                                                                 |
| `final_batch_time`        | `f64`  | The time when the last transaction of this interval was dispatched, as seconds since the load test **phase** started. Empty when no transactions were dispatched. |
| `phase`                   | string | One of `"warmup"`, `"pause"`, or `"load"`.                                                                                                                        |

---

## Transaction CSV

The transaction CSV provides one row for each individual transaction. It
records detailed timing and outcome information. Rows are written in the
order in which results are reported, not in execution order; sort by
`start_time` or `send_time` where order matters.

| Column              | Type    | Description                                                                                                                                                                                                                                                                                                                           |
| ------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `target_time`       | `f64`   | The deadline of the load interval this transaction was scheduled for.                                                                                                                                                                                                                                                                 |
| `start_time`        | `f64`   | The scheduled arrival time of this transaction, as seconds since the load test **phase** started.                                                                                                                                                                                                                                     |
| `send_time`         | `f64`   | The time when the request was sent, as seconds since the load test **phase** started. `send_time - start_time` is how late the request left the load generator. Empty for dropped transactions since no request was sent.                                                                                                             |
| `load_generator_id` | `u8`    | The ID of the load generator instance that issued this request.                                                                                                                                                                                                                                                                       |
| `virtual_user_id`   | `u32`   | The ID of the virtual user that executed this transaction. Note that IDs are unique per load generator instances, **not** globally. Empty for transactions dropped before they got a virtual user.                                                                                                                                    |
| `spec_id`           | `usize` | The request spec this transaction executed. Empty for dropped transactions.                                                                                                                                                                                                                                                           |
| `response_time_ms`  | `u64`   | End-to-end elapsed time from the scheduled arrival until the outcome was determined, including any wait for a free virtual user. Present for all outcomes including dropped.                                                                                                                                                          |
| `service_time_ms`   | `u64`   | Time spent communicating with the service. Empty for dropped transactions since no request was sent.                                                                                                                                                                                                                                  |
| `outcome`           | string  | One of `"success"`, `"failed"`, `"timeout"`, or `"dropped"`.                                                                                                                                                                                                                                                                          |
| `reason`            | string  | Reason for the outcome of the transaction. Empty for successful transactions. Dropped transactions are `NoVirtualUser` (no virtual user became free within 10 ms of the scheduled arrival), `Late` (the load generator got to the transaction more than 10 ms late), or `Error(...)` (the script failed before the request was sent). |

---

## Input Files

At the start of a run, the orchestrator copies its input files to the output
directory under its default input file names: `profile.csv`,
`script.lua`, `registry.yaml`, and `warmup.yaml`. Together with the run
metadata, they enable reproducing a run.

---

## Run Metadata

At the end of a completed run, the orchestrator writes `run.json`, which
describes the run, where it ran, and whether it is valid.

| Field                | Type   | Description                                                                       |
| -------------------- | ------ | --------------------------------------------------------------------------------- |
| `version`            | string | The CreoBench version of the orchestrator.                                        |
| `started_at`         | string | When the run started, in RFC 3339 format and UTC.                                 |
| `finished_at`        | string | When the run finished, in RFC 3339 format and UTC.                                |
| `valid`              | `bool` | `false` if any load-phase transaction was dropped, so the run is invalid.         |
| `virtual_user_count` | `u32`  | The number of virtual users per load generator instance.                          |
| `timeout_ms`         | `u64`  | The HTTP request timeout in milliseconds, or `null` if requests never time out.   |
| `seed`               | `u64`  | The seed of the random number generators.                                         |
| `generators`         | array  | The load generator instances, see below.                                          |
| `drops`              | object | The dropped transactions, only present if any transaction was dropped, see below. |

Each entry of `generators` describes one load generator instance:

| Field            | Type   | Description                                                                          |
| ---------------- | ------ | ------------------------------------------------------------------------------------ |
| `id`             | `u8`   | The ID of the instance, as in the `load_generator_id` column of the transaction CSV. |
| `version`        | string | The CreoBench version of the instance.                                               |
| `os`             | string | The operating system, e.g. `linux`.                                                  |
| `arch`           | string | The CPU architecture, e.g. `x86_64`.                                                 |
| `cores`          | `u32`  | The number of cores the instance may run on.                                         |
| `worker_threads` | `u32`  | The number of threads executing requests; one more core is used for pacing.          |

`drops` has a `load` and a `warmup` entry, the latter covering warmup and
pause. Each counts the `transactions` of the phase and how many were dropped
per reason: `no_virtual_user`, `late`, and `error` (see the `reason` column of
the transaction CSV).
