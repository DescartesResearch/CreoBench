# Best Practices

This page presents practical guidance for designing and running effective load tests with CreoBench. It builds on the
concepts introduced in [Concepts](concepts.md) and focuses on the decisions you make when preparing and executing a
real test.

If you are new to CreoBench, start with [Concepts](concepts.md) for an overview of the framework’s core building blocks.

## Table of Contents

- [Virtual User Count Sizing](#virtual-user-count-sizing)
- [Profile Design](#profile-design)
- [Choosing Warmup Values](#choosing-warmup-values)

---

## Virtual User Count Sizing

The `--virtual-user-count` flag sets the number of virtual users (VU) **per load generator instance**, not the total
across all instances. For example, running three load generators with `-u 100` creates 300 concurrent VUs in total.

When a VU starts a transaction, it is taken out of the generator’s VU pool until the transaction finishes. Once the
transaction completes, the VU is returned to the pool and can execute another transaction.

If all VUs are currently executing transactions, newly scheduled transactions must wait for a VU to become available.
If the wait exceeds the configured transaction timeout, the transaction is dropped instead of being executed. Dropped
transactions can be seen in the CSV outputs or in the console output during a load test.

A useful starting point for calculating the required number of virtual users to guarantee now queuing
is [Little’s Law](https://en.wikipedia.org/wiki/Little%27s_law):

```math
L = \lambda \times W
```

where:

- $L$ is the required number of VUs,
- $\lambda$ is the maximum transactions in seconds in your load profile, and
- $W$ is the expected average service time per transaction

For instance, if your load profile sends a maximum of $200$ transactions per second and the expected service time is
$250ms$, then you would need $200 \times 0.25 = 50$ virtual users in the pool. This calculation assumes perfect
conditions. In practice, configure additional virtual users as headroom for service time variation and delays introduced
by the load generator instance. For example a pool of about 60 virtual users may be a reasonable starting point for this
workload. Remember: The VU count is per instance, so running this load across two machines would only require 30 VUs
per load generator instance.

A large gap between response time (RT) and service time (ST) may be an indication of an exhausted or undersized VU pool.
When the RT of a transaction is substantially higher than its ST, the transaction may be spending much of its time
waiting for a free virtual user in the VU pool. However, a bottleneck on the load generator machine, such as CPU, memory,
or network saturation, can also cause a large gap between ST and RT. Thus, a large gap does not necessarily indicate an
undersized VU pool, but may imply the load generator machine's resources are insufficient for the target load.

---

## Profile Design

A load profile defines how many transactions to send during each period of a test, and its shape determines what you
learn about the system. See [Load profiles](load-profiles.md) for the CSV format and validation rules.

In most cases, begin at a low rate and increase it gradually rather than jumping directly to peak load. This gives you
a clearer view of how the system responds as demand increases and helps identify when response times begin to degrade.
To make those observations meaningful, give each load interval enough time for the system to respond. Intervals that are
too short can produce noisy results because the system may still be reacting to the previous step. For capacity tests,
follow the ramp with a sustained interval at the target rate and run it long enough to observe the system’s behaviour
over time.

The best profile depends on the question you want to answer. A ramp shows when latency begins to climb, a step profile
shows how the system behaves at specific load levels, a spike tests recovery from a burst, and a steady profile measures
sustainable throughput over time.

---

## Choosing Warmup Values

A well-configured warmup phase primes caches, warms up JIT compilers, and establishes connection pools before the main
load test begins. Poor warmup settings can make your results reflect cold-start penalties instead of steady-state
performance. See [Warmup](warmup.md) for the full configuration reference.

### Match the warmup rate to the load test

A well-configured warmup phase prepares the system before measurements begin, helping the main load test reflect normal
operating conditions rather than startup behaviour. See [Warmup](warmup.md) for the full configuration reference.

Choose a warmup `rate` that is high enough to prepare the system for the main load test without overwhelming it at the
start. In general, the warmup should be long enough for every VU to execute several transactions. We recommend a
`duration` of at least 120 seconds. If the system cannot handle a high warmup rate initially, use a lower rate and
increase the duration so that the system still has enough time to become ready.

After the warmup, set `pause` long enough for all in-flight warmup requests to finish, with some additional headroom for
the system to settle. As a rule of thumb, choose a pause that is slightly longer than the configured transaction
timeout.

---

If something goes wrong, see [Troubleshooting](troubleshooting.md).
