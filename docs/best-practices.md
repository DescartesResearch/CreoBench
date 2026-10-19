# Best Practices

This page presents practical guidance for designing and running effective load tests with CreoBench. It builds on the
concepts introduced in [Concepts](concepts.md) and focuses on the decisions you make when preparing and executing a
real test.

If you are new to CreoBench, start with [Concepts](concepts.md) for an overview of the framework’s core building blocks.

## Table of Contents

- [Virtual User Count Sizing](#virtual-user-count-sizing)
- [Load Generator Resources](#load-generator-resources)
- [Profile Design](#profile-design)
- [Choosing Warmup Values](#choosing-warmup-values)

---

## Virtual User Count Sizing

The `--virtual-user-count` flag sets the number of virtual users (VU) **per load generator instance**, not the total
across all instances. For example, running three load generators with `-u 100` creates 300 concurrent VUs in total.

When a VU starts a transaction, it is taken out of the generator’s VU pool until the transaction finishes. Once the
transaction completes, the VU is returned to the pool and can execute another transaction.

Transactions are scheduled independently of whether a VU is free. If all VUs are busy, a newly scheduled transaction
waits for the next free one, but at most 10 ms. If no VU becomes free in time, the transaction is dropped
(`NoVirtualUser`), and since the scheduled load was not delivered, the run is invalid. Size the pool so that this
practically never happens.

On average, a load generator instance needs $a = \lambda \times W$ busy VUs
([Little’s Law](https://en.wikipedia.org/wiki/Little%27s_law)), where:

- $\lambda$ is the peak transaction rate per second **of this instance** in your load profile, and
- $W$ is the mean time a VU is busy per transaction: the expected response time of the system under test, plus any
  time spent in the script.

Because arrivals are random, the number of transactions in progress fluctuates around $a$ with a standard deviation of
$\sqrt{a}$, so the pool must cover the peaks, not the average:

```math
\text{VUs} = a + 7\sqrt{a}
```

This keeps the chance of even a single drop below 1% for runs of up to a day. For short exploratory runs of about a
minute, $a + 5\sqrt{a}$ suffices, but then a run is drop-free only with a probability of about 95%, and longer runs
almost certainly drop transactions.

For instance, if your load profile sends a maximum of $200$ transactions per second and the expected response time is
$250ms$, then $a = 200 \times 0.25 = 50$ and you need $50 + 7\sqrt{50} \approx 100$ VUs. The relative headroom shrinks
for larger pools: at $10{,}000$ transactions per second and $92ms$, $a = 920$ and $1{,}133$ VUs suffice, only 23% more.
Remember that the VU count is per instance: running the first example across two machines halves $\lambda$ per instance,
so $a = 25$ and each instance needs $25 + 7\sqrt{25} = 60$ VUs.

If you set `--timeout`, a VU is never busy for longer than the timeout. Sizing with $W$ equal to the timeout guarantees
that the pool never runs out, however slow the system under test gets; slow responses then show up as timeouts instead
of drops. Sizing with the expected response time needs fewer VUs, but drops if the system under test responds more
slowly than expected.

---

## Load Generator Resources

Each load generator instance reserves one CPU core for pacing. A dedicated thread dispatches every transaction at its
scheduled arrival time and busy-waits for the last millisecond before each arrival, so this core is fully busy at rates
of a few thousand transactions per second and above. The remaining cores execute requests. Run each instance on a
machine with at least two cores; with a single core, pacing competes with request execution and CreoBench logs a
warning. If the cores of one machine are not sufficient for the target load, split the load across multiple load
generator instances or lower it.

To check whether a load generator keeps up, compare `send_time` and `start_time` in the
[transaction CSV](outputs.md#transaction-csv). Their difference is the time a transaction spent in the load generator
before its request was sent, including any wait for a free virtual user.

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
timeout. If warmup transactions are still in flight when the pause ends, CreoBench logs a warning, since they still
occupy virtual users and are reported in the load phase.

---

If something goes wrong, see [Troubleshooting](troubleshooting.md).
