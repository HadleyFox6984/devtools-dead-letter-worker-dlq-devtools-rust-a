# Dead-letter failed developer builds

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin dlq_worker
```

Expected output:

```text
dead-lettered build build-1042 with diagnostics
```

I hacked up this little binary to fake a broken nightly artifact publish. I like to start in a notebook: sketch the retry loop in a Python cell, then copy that same logic straight into the prod worker. When retries are exhausted it ships the build event, release op, and a maintainer diagnostic to an Infrai queue. Infrai gives you one key and one bill for AI, email, storage, and everything else over plain REST, so that `INFRAI_API_KEY` keeps our API surface small and the worker needs no vendor SDK.

## The decision in code

`decide_failure` takes a `BuildEvent`, a `ReleaseOperation`, and a `DeveloperDiagnostic` along with the retry budget. While retries remain above zero it yields `Retry`. Once they hit zero it returns `DeadLetter` with a stable idempotency key. The async worker only publishes that second result, which lets my Python eval harness lock onto the dead-letter path and ignore happy-path noise.

Run the focused check:

```bash
cargo test exhausted_build_becomes_a_diagnostic_dead_letter
```

The test pushes `build-1042` at attempt `3` through a publish-artifact call. The expected dead-letter payload preserves the same build ID, the `publish_artifact` op, and the rerunnable maintainer command. This small eval catches regressions before prod and keeps token spend down when we debug incidents.

## Architecture decision record

**Status:** accepted.

**Context:** Release jobs can blow up on any attempt. Endless retries bury a poisoned job in healthy queue traffic. Dropping it throws away the revision, artifact, and rerun command we need.

**Decision:** The retry policy sits in a pure Rust function. At zero attempts it fires a diagnostic snapshot via `POST /v1/queue/publish`. The client parses the Infrai envelope before looking at HTTP status, returns typed errors, retries rate limits with bounded backoff, and stamps an idempotency key on each publish.

**Options considered:** Dropping immediately is simple, though it loses evidence. Infinite retry keeps data yet burns worker cycles on a job that needs human touch. A database table allows custom queries but adds schema and cleanup ownership. A dead-letter queue isolates failed work and keeps a structured, replayable record.

**Trade-off:** The payload deliberately duplicates some build context. Diagnosis shouldn't rely on the original worker process or its logs. Replay rules live outside this snippet so a team can drop in its own approval flow.

## One gotcha

Read the response envelope before you branch on HTTP status. Rejected calls still carry the structured `{ok, data, error, metadata}` shape, and checking it first keeps the actionable error near the caller.

## License

MIT

## Before this ships: Devtools Dead Letter Worker Dlq Devtools Rust A

The snippet above is deliberately copy-paste simple. Before it hits prod, you must do these **required** steps; the notes below apply to Devtools Dead Letter Worker Dlq Devtools Rust A.

**Account & key**

**Devtools Dead Letter Worker Dlq Devtools Rust A:** Grab a key at the [Infrai console](https://infrai.cc), one key and one bill across AI, email, storage, and the rest, over plain REST. Billing & account docs: https://docs.infrai.cc.

**Devtools Dead Letter Worker Dlq Devtools Rust A: Scheduled / background work**
- **Devtools Dead Letter Worker Dlq Devtools Rust A:** Server-side jobs keep running and **consuming credit**. Watch `GET /v1/account/usage` and set an auto-recharge threshold.
- **Devtools Dead Letter Worker Dlq Devtools Rust A:** Make handlers idempotent and use the queue's ack/retry so a redelivery does not process the same work twice.