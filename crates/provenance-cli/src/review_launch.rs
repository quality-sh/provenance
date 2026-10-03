use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const CODE_LIFETIME: Duration = Duration::from_secs(120);
const ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_CODES: usize = 128;
const MAX_ATTEMPTS: usize = 64;
const FRESH_LINK: &str =
    "Connection refused. Run `provenance <record-id> --review-link` to get a fresh link.";

#[derive(Clone)]
pub struct LaunchCodes {
    bearer: Arc<str>,
    repository_id: Arc<str>,
    scope: Arc<str>,
    instance_nonce: Arc<str>,
    state: Arc<Mutex<LaunchState>>,
}

#[derive(Default)]
struct LaunchState {
    codes: VecDeque<LaunchCode>,
    attempts: VecDeque<Instant>,
}

struct LaunchCode {
    value: String,
    expires_at: Instant,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintRequest {
    repository_id: String,
    scope: String,
    instance_nonce: String,
}

#[derive(Deserialize)]
pub struct ExchangeRequest {
    code: String,
}

enum ExchangeError {
    Refused,
    RateLimited,
}

impl LaunchCodes {
    pub fn new(bearer: String, repository_id: String, scope: String, instance_nonce: String) -> Self {
        Self {
            bearer: bearer.into(),
            repository_id: repository_id.into(),
            scope: scope.into(),
            instance_nonce: instance_nonce.into(),
            state: Arc::new(Mutex::new(LaunchState::default())),
        }
    }

    pub fn mint(&self) -> String {
        self.mint_at(Instant::now())
    }

    fn mint_at(&self, now: Instant) -> String {
        let mut state = self.state.lock().expect("launch-code state lock");
        discard_expired(&mut state.codes, now);
        while state.codes.len() >= MAX_CODES {
            state.codes.pop_front();
        }
        let value = uuid::Uuid::new_v4().simple().to_string();
        state.codes.push_back(LaunchCode {
            value: value.clone(),
            expires_at: now + CODE_LIFETIME,
        });
        value
    }

    fn exchange(&self, code: &str) -> Result<String, ExchangeError> {
        self.exchange_at(code, Instant::now())
    }

    fn exchange_at(&self, code: &str, now: Instant) -> Result<String, ExchangeError> {
        let mut state = self.state.lock().expect("launch-code state lock");
        discard_expired(&mut state.codes, now);
        while state
            .attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= ATTEMPT_WINDOW)
        {
            state.attempts.pop_front();
        }
        if state.attempts.len() >= MAX_ATTEMPTS {
            return Err(ExchangeError::RateLimited);
        }
        state.attempts.push_back(now);
        let Some(index) = state.codes.iter().position(|entry| entry.value == code) else {
            return Err(ExchangeError::Refused);
        };
        state.codes.remove(index);
        drop(state);
        Ok(self.bearer.to_string())
    }

    fn matches(&self, request: &MintRequest) -> bool {
        request.repository_id == self.repository_id.as_ref()
            && request.scope == self.scope.as_ref()
            && request.instance_nonce == self.instance_nonce.as_ref()
    }
}

fn discard_expired(codes: &mut VecDeque<LaunchCode>, now: Instant) {
    codes.retain(|entry| entry.expires_at > now);
}

pub async fn mint(State(codes): State<LaunchCodes>, Json(request): Json<MintRequest>) -> Response {
    if !codes.matches(&request) {
        return (StatusCode::FORBIDDEN, FRESH_LINK).into_response();
    }
    Json(json!({ "code": codes.mint() })).into_response()
}

pub async fn exchange(
    State(codes): State<LaunchCodes>,
    Json(request): Json<ExchangeRequest>,
) -> Response {
    match codes.exchange(&request.code) {
        Ok(bearer) => Json(json!({ "bearer": bearer })).into_response(),
        Err(ExchangeError::Refused) => (StatusCode::UNAUTHORIZED, FRESH_LINK).into_response(),
        Err(ExchangeError::RateLimited) => (StatusCode::TOO_MANY_REQUESTS, FRESH_LINK).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> LaunchCodes {
        LaunchCodes::new(
            "bearer-secret".to_owned(),
            "local".to_owned(),
            "default".to_owned(),
            "host-nonce".to_owned(),
        )
    }

    #[test]
    fn expired_code_is_refused() {
        let codes = registry();
        let minted_at = Instant::now();
        let code = codes.mint_at(minted_at);

        assert!(matches!(
            codes.exchange_at(&code, minted_at + CODE_LIFETIME),
            Err(ExchangeError::Refused)
        ));
    }

    #[test]
    fn exchange_attempts_are_bounded() {
        let codes = registry();
        let now = Instant::now();
        for _ in 0..MAX_ATTEMPTS {
            assert!(matches!(
                codes.exchange_at("invalid", now),
                Err(ExchangeError::Refused)
            ));
        }
        assert!(matches!(
            codes.exchange_at("invalid", now),
            Err(ExchangeError::RateLimited)
        ));
    }
}
