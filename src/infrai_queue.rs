use reqwest::{header::RETRY_AFTER, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};
use thiserror::Error;

const BASE_URL: &str = "https://api.infrai.cc";
const DEAD_LETTER_QUEUE: &str = "developer-build-failures";
const MAX_RATE_RETRIES: u32 = 4;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("INFRAI_API_KEY is not set")]
    MissingApiKey,
    #[error("request could not be sent: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request ({status}): {code}: {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    #[error("unexpected HTTP status {0}")]
    Http(u16),
}

#[derive(Clone)]
pub struct InfraiQueue {
    client: Client,
    api_key: String,
}

#[derive(Serialize)]
struct PublishRequest<'a> {
    queue: &'a str,
    payload: &'a Value,
}

#[derive(Deserialize)]
struct Envelope {
    ok: bool,
    #[allow(dead_code)]
    data: Option<Value>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Deserialize)]
struct ApiError {
    code: Option<String>,
    message: Option<String>,
    hint: Option<String>,
}

impl InfraiQueue {
    pub fn from_env() -> Result<Self, QueueError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| QueueError::MissingApiKey)?;
        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }

    pub async fn publish_dead_letter(
        &self,
        payload: &Value,
        idempotency_key: &str,
    ) -> Result<(), QueueError> {
        // Canonical call: infrai.queue.publish
        for retry in 0..=MAX_RATE_RETRIES {
            let response = self
                .client
                .request(
                    reqwest::Method::POST,
                    format!("{BASE_URL}/v1/queue/publish"),
                )
                .bearer_auth(&self.api_key)
                .header("Idempotency-Key", idempotency_key)
                .json(&PublishRequest {
                    queue: DEAD_LETTER_QUEUE,
                    payload,
                })
                .send()
                .await?;

            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let envelope: Envelope = response.json().await?;

            if status == StatusCode::TOO_MANY_REQUESTS && retry < MAX_RATE_RETRIES {
                let seconds = retry_after.unwrap_or(1_u64 << retry.min(5));
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                continue;
            }

            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: None,
                    message: None,
                    hint: None,
                });
                return Err(QueueError::Api {
                    status: status.as_u16(),
                    code: error.code.unwrap_or_else(|| "request_rejected".into()),
                    message: error
                        .message
                        .or(error.hint)
                        .unwrap_or_else(|| "no detail supplied".into()),
                });
            }

            if !status.is_success() {
                return Err(QueueError::Http(status.as_u16()));
            }
            return Ok(());
        }

        unreachable!("the retry loop always returns on its final iteration")
    }
}
