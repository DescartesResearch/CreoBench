# Warmup

A load test's warmup phase runs a short period of traffic before the main load test
begins. Its purpose is to prepare the system for the test by priming
caches, establishing connection pools, and warming up JIT compilers. This
helps ensure that the main test results reflect steady-state performance
rather than cold-start effects.

If you are new to CreoBench, start with [Concepts](concepts.md) for an
overview of how the warmup phase fits into a load test.

## Table of Contents

- [Warmup Configuration](#warmup-configuration)
- [Understanding the Warmup Phase](#understanding-the-warmup-phase)
- [Choosing Warmup Values](#choosing-warmup-values)

---

## Warmup Configuration

The warmup phase is configured in a YAML file with three fields:

| Field      | Type | Description                                                          |
| ---------- | ---- | -------------------------------------------------------------------- |
| `rate`     | u32  | Target transactions per second during the warmup phase.              |
| `duration` | u32  | Length of the warmup phase in seconds.                               |
| `pause`    | u32  | Duration of the pause between the warmup and load phases in seconds. |

The file must contain all three fields. For example:

```yaml
rate: 10
duration: 30
pause: 5
```

Pass the warmup file to the orchestrator with the `--warmup` flag:

```sh
creo-orch \
  --warmup warmup.yaml \
  ...
```

If `--warmup` is not provided, the orchestrator looks for `warmup.yaml` in the
current directory.

---

## Understanding the Warmup Phase

A warmup configuration creates three distinct periods in the load test:

1. **Warmup phase**: Runs for `duration` seconds, sending traffic at the
   configured `rate`. During this phase, the interval CSV includes rows with
   `phase=warmup`. The console output shows negative time values, indicating
   time remaining before the load phase begins.

2. **Pause phase**: A period lasting `pause` seconds where no traffic
   is sent. During this phase, the interval CSV includes rows with `phase=pause`
   and `load_level=0`. The pause allows the system to settle after the warmup
   traffic and allows transactions sent during the warmup phase to complete
   before measurements begin.

3. **Load phase** — The main load test begins after the pause, using the
   profile you specified.

---

## Choosing Warmup Values

Select warmup values based on the characteristics of your system under test:

**Rate** — Use a rate representative of your target workload. The warmup
rate should be similar to the steady-state rate of the main load test. If
you use a much lower rate during warmup, caches may not be fully primed.
If you use a much higher rate, you may prematurely exhaust resources.

**Duration** — Choose a duration long enough for your system to reach steady
state. We recommend choosing a duration of at least 60 seconds.

**Pause** — The pause should be long enough for the system to settle but
short enough to avoid cooling down. Typical values are 2–10 seconds. A
pause of 0 disables the pause period entirely.
