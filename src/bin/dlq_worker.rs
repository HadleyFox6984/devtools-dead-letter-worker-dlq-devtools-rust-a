use devtools_dead_letter_worker::{
    infrai_queue::InfraiQueue,
    queue_worker::{
        decide_failure, BuildEvent, DeveloperDiagnostic, FailureDecision, ReleaseOperation,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event = BuildEvent {
        job_id: "build-1042".into(),
        repository: "acme/compiler".into(),
        revision: "8f31c2a".into(),
        attempt: 3,
    };
    let release = ReleaseOperation::PublishArtifact {
        artifact: "compiler-linux-x86_64.tar.gz".into(),
        channel: "nightly".into(),
    };
    let diagnostic = DeveloperDiagnostic {
        summary: "artifact checksum step failed".into(),
        command: "cargo xtask release".into(),
        log_excerpt: "checksum mismatch for release archive".into(),
    };

    match decide_failure(&event, &release, &diagnostic, 3) {
        FailureDecision::Retry { next_attempt } => {
            println!("retry build {} at attempt {next_attempt}", event.job_id);
        }
        FailureDecision::DeadLetter {
            payload,
            idempotency_key,
        } => {
            InfraiQueue::from_env()?
                .publish_dead_letter(&payload, &idempotency_key)
                .await?;
            println!("dead-lettered build {} with diagnostics", event.job_id);
        }
    }

    Ok(())
}
