//! Only two fixed HTTP routes on a literal IPv4 loopback address. No DNS, proxy, redirects,
//! cookies, compression, remote endpoints, downloads, tools or LM Studio integrations.
use crate::{Answer, Plan, parse_answer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use zeroize::Zeroize;

pub const RESPONSE_BYTES: usize = 64 * 1024;

pub struct Config {
    pub port: u16,
    pub model: String,
    pub token: String,
}
impl Drop for Config {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(180))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|_| "Could not start the local HTTP client.".into())
}

async fn exchange(config: &Config, plan: Option<&Plan>) -> Result<serde_json::Value, String> {
    if config.port == 0 {
        return Err("Choose a local server port from 1 to 65535.".into());
    }
    if config.token.len() > 4096 || config.token.chars().any(char::is_control) {
        return Err("Invalid authentication token.".into());
    }
    let base = format!("http://127.0.0.1:{}", config.port);
    let http = client()?;
    let mut request = match plan {
        None => http.get(format!("{base}/v1/models")).timeout(Duration::from_secs(10)),
        Some(plan) => {
            if config.model.trim().is_empty() || config.model.len() > 256 || config.model.chars().any(char::is_control) {
                return Err("Choose a model returned by the local server.".into());
            }
            http.post(format!("{base}/v1/chat/completions")).json(&serde_json::json!({
                "model":config.model,"messages":[{"role":"system","content":crate::SYSTEM_PROMPT},{"role":"user","content":plan.user_message}],
                "stream":false,"temperature":0.2,"max_tokens":1024
            }))
        }
    };
    if !config.token.is_empty() {
        request = request.bearer_auth(&config.token);
    }
    let mut response = request.send().await.map_err(|_| {
        "Cannot reach the local model server, or the request timed out. Start LM Studio's local server, check its port and load a chat model."
            .to_string()
    })?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            300..=399 => "The server requested a redirect. Redirects are blocked.".into(),
            401 | 403 => "Local server authentication failed. Enter its API token, if required.".into(),
            400 | 413 | 422 => "The model rejected the request. Check its chat template and context size, or preview a smaller scope.".into(),
            code => {
                format!("Local server returned HTTP {code}. Check the model is available locally. Server error bodies are not displayed or logged.")
            }
        });
    }
    if response.content_length().is_some_and(|n| n > RESPONSE_BYTES as u64) {
        return Err("The local server response exceeded the 64 KiB limit.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Could not finish reading the local response.")? {
        if bytes.len().saturating_add(chunk.len()) > RESPONSE_BYTES {
            return Err("The local server response exceeded the 64 KiB limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "The local server returned invalid JSON.".into())
}

/// The caller runs this on one background thread. Cancellation aborts the HTTP future;
/// server-side inference may take longer to stop. No late result reaches the panel.
fn run<T: Send + 'static>(cancel: Cancel, work: impl std::future::Future<Output = Result<T, String>> + Send + 'static) -> Result<T, String> {
    if cancel.is_cancelled() {
        return Err("Cancelled.".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_| "Could not start the local request worker.")?;
    runtime.block_on(async move {
        let job = tokio::spawn(work);
        loop {
            if cancel.is_cancelled() {
                job.abort();
                let _ = job.await;
                return Err("Cancelled. The local server may still be stopping inference.".into());
            }
            if job.is_finished() {
                return job.await.map_err(|_| "The local request worker stopped unexpectedly.")?;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
}

pub fn models(config: Config, cancel: Cancel) -> Result<Vec<String>, String> {
    run(cancel, async move {
        let response = exchange(&config, None).await?;
        let data = response["data"].as_array().ok_or("The server returned no model list.")?;
        let mut out: Vec<_> = data
            .iter()
            .filter_map(|m| m["id"].as_str())
            .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
            .map(str::to_string)
            .take(256)
            .collect();
        out.sort();
        out.dedup();
        if out.is_empty() { Err("No models are available. Load an on-disk chat model in LM Studio first.".into()) } else { Ok(out) }
    })
}

pub fn ask(config: Config, plan: Plan, cancel: Cancel) -> Result<Answer, String> {
    run(cancel, async move {
        let response = exchange(&config, Some(&plan)).await?;
        parse_answer(response, &plan)
    })
}
