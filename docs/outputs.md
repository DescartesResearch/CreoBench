# Outputs

CreoBench produces three output formats during and after a load test: a live
console progress line, an interval CSV summarizing each load interval, and a
transaction CSV recording every individual transaction. This page documents
each format so you can analyze your results.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how the reporting system fits into a load test.

## Table of Contents

- [Console Output](#console-output)
- [Interval CSV](#interval-csv)
- [Transaction CSV](#transaction-csv)

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

| Column                    | Type   | Description                                                                                                                           |
| ------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `target_time`             | `f64`  | The target time of the load interval, as seconds since the load test **phase** started.                                               |
| `load_level`              | `u32`  | The number of transactions scheduled for this interval.                                                                               |
| `successful_transactions` | `u64`  | Number of transactions that completed successfully.                                                                                   |
| `failed_transactions`     | `u64`  | Number of transactions that failed.                                                                                                   |
| `timeout_transactions`    | `u64`  | Number of transactions that timed out.                                                                                                |
| `dropped_transactions`    | `u64`  | Number of transactions that were dropped.                                                                                             |
| `avg_service_time`        | `u64`  | Average service time of non-dropped transactions.                                                                                     |
| `final_batch_time`        | `f64`  | The time when the last batch was dispatched, as seconds since the load test **phase** started. Empty when no batches were dispatched. |
| `phase`                   | string | One of `"warmup"`, `"pause"`, or `"load"`.                                                                                            |

---

## Transaction CSV

The transaction CSV provides one row for each individual transaction. It
records detailed timing and outcome information.

| Column              | Type    | Description                                                                                                                                 |
| ------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `target_time`       | `f64`   | The deadline of the load interval this transaction was scheduled for.                                                                       |
| `start_time`        | `f64`   | The time when this transaction actually started, as seconds since the load test started.                                                    |
| `load_generator_id` | `u8`    | The ID of the load generator instance that issued this request.                                                                             |
| `virtual_user_id`   | `u32`   | The ID of the virtual user that executed this transaction. Note that IDs are unique per load generator instances, **not** globally.         |
| `spec_id`           | `usize` | The request spec this transaction executed. Empty for dropped transactions.                                                                 |
| `response_time_ms`  | `u64`   | End-to-end elapsed time from when the transaction was created until its outcome was determined. Present for all outcomes including dropped. |
| `service_time_ms`   | `u64`   | Time spent communicating with the service. Empty for dropped transactions since no request was sent.                                        |
| `outcome`           | string  | One of `"success"`, `"failed"`, `"timeout"`, or `"dropped"`.                                                                                |
| `reason`            | string  | Reason for the outcome of the transaction. Empty for successful transactions.                                                               |
