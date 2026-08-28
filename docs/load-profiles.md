# Load Profiles

A load profile tells CreoBench how much traffic to generate and when. This
page explains the CSV format, shows common profile shapes, and describes the
validation rules that apply.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how load profiles fit into a load test.

## Table of Contents

- [Load Profile Format](#load-profile-format)
- [Understanding Load Intervals and Transaction Pacing](#understanding-load-intervals-and-transaction-pacing)
- [Load Profile Validation](#load-profile-validation)

---

## Load Profile Format

A load profile is a CSV file with two columns:

| Column     | Type | Description                                         |
| ---------- | ---- | --------------------------------------------------- |
| `deadline` | f64  | Time in seconds from the start of the load test.    |
| `count`    | u32  | Number of transactions to send during the interval. |

The file must have a header row. Each subsequent row defines one step in the
profile. For example:

```csv
deadline,count
1.0,10
2.0,20
3.0,20
```

---

## Understanding Load Intervals and Transaction Pacing

**Load intervals** are the building blocks of your profile. Each interval represents
the time window between two consecutive deadlines, and CreoBench distributes your
transactions across that period using an exponential distribution. This mimics
real-world traffic patterns where request arrivals are independent events and
the timing of one request doesn't influence the next.

The **first interval** starts at t=0 (the beginning of the load phase) and runs
until your first deadline. Each subsequent interval spans from one deadline to
the next. During each interval, CreoBench sends exactly the number of transactions
specified in the `count` column

Consider the profile from above:

- From 0s to 1s, CreoBench sends 10 transactions (10 TPS).
- From 1s to 2s, CreoBench sends 20 transactions (20 TPS).
- From 2s to 3s, CreoBench sends 20 transactions (20 TPS).

Deadlines are absolute, not relative to the previous step. A deadline of
`3.0` always means three seconds after the load test started, regardless of
the previous deadline.

When you run multiple load generator instances, the orchestrator spreads each
step's transaction count across all instances. Every instance follows the
load profile's timeline.

---

## Load Profile Validation

Your load profile must satisfy several constraints. If any constraint is violated,
the load test won't begin and you'll receive a clear error message. These rules
exist to ensure your profile represents a valid load scenario.

**Deadlines must be positive.** Negative deadlines are part of the [Warmup](warmup.md).

**Deadlines must be strictly increasing.** Each deadline must be greater than the
previous one. This ensures your profile progresses forward in time without
gaps or reversals.

**The profile must contain at least one step.** A profile needs at least one step to
define the load to generate.
