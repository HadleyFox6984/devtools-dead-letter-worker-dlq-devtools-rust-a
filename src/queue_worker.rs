use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildEvent {
    pub job_id: String,
    pub repository: String,
    pub revision: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ReleaseOperation {
    PublishArtifact { artifact: String, channel: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeveloperDiagnostic {
    pub summary: String,
    pub command: String,
    pub log_excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureDecision {
    Retry {
        next_attempt: u32,
    },
    DeadLetter {
        payload: Value,
        idempotency_key: String,
    },
}

pub fn decide_failure(
    event: &BuildEvent,
    release: &ReleaseOperation,
    diagnostic: &DeveloperDiagnostic,
    max_attempts: u32,
) -> FailureDecision {
    if event.attempt < max_attempts {
        return FailureDecision::Retry {
            next_attempt: event.attempt + 1,
        };
    }

    FailureDecision::DeadLetter {
        payload: json!({
            "kind": "developer_tools_job_failed",
            "build": event,
            "release": release,
            "diagnostic": diagnostic,
            "attempts_exhausted": event.attempt
        }),
        idempotency_key: format!("devtools-dlq:{}:{}", event.job_id, event.attempt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (BuildEvent, ReleaseOperation, DeveloperDiagnostic) {
        (
            BuildEvent {
                job_id: "build-1042".into(),
                repository: "acme/compiler".into(),
                revision: "8f31c2a".into(),
                attempt: 3,
            },
            ReleaseOperation::PublishArtifact {
                artifact: "compiler-linux-x86_64.tar.gz".into(),
                channel: "nightly".into(),
            },
            DeveloperDiagnostic {
                summary: "artifact checksum step failed".into(),
                command: "cargo xtask release".into(),
                log_excerpt: "checksum mismatch for release archive".into(),
            },
        )
    }

    #[test]
    fn exhausted_build_becomes_a_diagnostic_dead_letter() {
        let (event, release, diagnostic) = fixture();

        let decision = decide_failure(&event, &release, &diagnostic, 3);

        let FailureDecision::DeadLetter {
            payload,
            idempotency_key,
        } = decision
        else {
            panic!("expected a dead-letter decision");
        };
        assert_eq!(payload["build"]["job_id"], "build-1042");
        assert_eq!(payload["release"]["operation"], "publish_artifact");
        assert_eq!(payload["diagnostic"]["command"], "cargo xtask release");
        assert_eq!(idempotency_key, "devtools-dlq:build-1042:3");
    }

    #[test]
    fn remaining_budget_requests_another_attempt() {
        let (mut event, release, diagnostic) = fixture();
        event.attempt = 2;

        assert_eq!(
            decide_failure(&event, &release, &diagnostic, 3),
            FailureDecision::Retry { next_attempt: 3 }
        );
    }
}
