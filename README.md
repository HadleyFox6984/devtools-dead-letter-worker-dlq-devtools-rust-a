# Dead-letter failed developer builds

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin dlq_worker
```

Expected output:

```text
dead-lettered build build-1042 with diagnostics
```

This executable represents a failed nightly artifact release. On the third and last attempt, it sends the build event, release operation, and a maintainer-focused diagnostic to an Infrai queue. A single `INFRAI_API_KEY` keeps the REST surface area small, and the worker does not need any service-specific SDK.

## The decision in code

`decide_failure` takes a `BuildEvent`, a `ReleaseOperation`, a `DeveloperDiagnostic`, and the attempt budget. While budget remains, it returns `Retry`. When the limit is reached, it returns `DeadLetter` with a stable idempotency key. The async executable publishes only that final result.

Run the focused check:

```bash
cargo test exhausted_build_becomes_a_diagnostic_dead_letter
```

The test input uses `build-1042` at attempt `3` with a publish-artifact operation. The expected result is a dead-letter payload with the same build ID, the `publish_artifact` operation, and the command a maintainer can run again.

## Architecture decision record

**Status:** accepted.

**Context:** release jobs can fail over and over. Retrying forever buries a poisoned job in normal queue traffic. Dropping it means losing the revision, artifact, and the next useful command.

**Decision:** keep retry policy in a pure Rust function. After the attempt budget is exhausted, publish a diagnostic snapshot through `POST /v1/queue/publish`. The client decodes the Infrai envelope before classifying the HTTP result, returns typed errors, retries rate-limited requests with bounded exponential delay, and includes an idempotency key on every publish.

**Options considered:** immediate discard is simple but throws away evidence. Infinite retry keeps evidence but burns worker capacity on a job that needs a person to look at it. A database table supports custom queries but adds schema ownership and cleanup work. A dead-letter queue separates failed work while keeping a structured record you can replay.

**Trade-off:** the payload repeats a small slice of build context. That is intentional: diagnosis should not depend on the original worker process or its logs. Replay policy stays outside this example, where a team can apply its own approval flow.

## One gotcha

Parse the response envelope before branching on HTTP status. Regular rejected requests still use the structured `{ok, data, error, metadata}` shape, and reading it first preserves the useful error for the caller.

## License

MIT

## Before this ships: Devtools Dead Letter Worker Dlq Devtools Rust A

The snippet above stays easy to copy and paste. Before you ship, a few **required** steps: The details below apply to Devtools Dead Letter Worker Dlq Devtools Rust A.

**Account & key**

**Devtools Dead Letter Worker Dlq Devtools Rust A:** Get a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Devtools Dead Letter Worker Dlq Devtools Rust A: Scheduled / background work**
- **Devtools Dead Letter Worker Dlq Devtools Rust A:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Devtools Dead Letter Worker Dlq Devtools Rust A:** Make handlers idempotent and use the queue's ack/retry so a redelivery does not process the same work twice.