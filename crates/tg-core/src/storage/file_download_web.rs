//! Web queue/progress state, with real reqwest HTTP over the owner's runtime.
//! No private event loop and no network activity in tests.
use super::file_download::{FailureReason, FileLoader, MAX_FILE_IN_MEMORY};
use std::collections::{BTreeMap, VecDeque};
pub const MAX_WEB_FILE_QUERIES: usize = 8;
pub const MAX_HTTP_REDIRECTS: i32 = 5;
pub const RESET_DOWNLOAD_PRIORITIES_TIMEOUT: i64 = 200;
pub const MAX_WEB_FILE: i64 = 4000 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WebRequestType {
    #[default]
    FullLoad,
    OnlySize,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Progress {
        ready: i64,
        total: i64,
        streamed: Vec<u8>,
    },
    Finished(Vec<u8>),
    Error,
}
#[derive(Clone, Debug, PartialEq)]
pub enum WebEffect {
    Get { id: u64, reply: u64, url: String },
    Abort { reply: u64 },
    ScheduleReset { at: i64 },
    Update { id: u64, data: Update },
}
#[derive(Clone, Debug)]
struct Enqueued {
    id: u64,
    url: String,
    stream: bool,
}
struct Sent {
    url: String,
    reply: u64,
    stream: bool,
    data: Vec<u8>,
    ready: i64,
    total: i64,
    redirects_left: i32,
}
#[derive(Default)]
pub struct WebLoadManager {
    queue: VecDeque<Enqueued>,
    previous_generation: VecDeque<Enqueued>,
    sent: BTreeMap<u64, Sent>,
    next_reply: u64,
    reset_at: Option<i64>,
    effects: Vec<WebEffect>,
}
pub fn is_supported_web_url(url: &str) -> bool {
    // QUrl accepts a syntactically valid empty authority; HTTP later rejects it.
    reqwest::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
        || url.split_once(':').is_some_and(|(scheme, rest)| {
            (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
                && rest.chars().all(|c| c == '/')
        })
}
impl WebLoadManager {
    pub fn enqueue(&mut self, id: u64, url: String, stream: bool, now: i64) {
        if self.queue.iter().any(|e| e.id == id) {
            return;
        }
        if !is_supported_web_url(&url) {
            self.effects.push(WebEffect::Update {
                id,
                data: Update::Error,
            });
            return;
        }
        self.previous_generation.retain(|e| e.id != id);
        self.queue.push_back(Enqueued { id, url, stream });
        if self.reset_at.is_none() {
            let at = now.saturating_add(RESET_DOWNLOAD_PRIORITIES_TIMEOUT);
            self.reset_at = Some(at);
            self.effects.push(WebEffect::ScheduleReset { at });
        }
        self.check_send_next();
    }
    pub fn remove(&mut self, id: u64) {
        self.queue.retain(|e| e.id != id);
        self.previous_generation.retain(|e| e.id != id);
        self.remove_sent(id);
    }
    pub fn reset_generation(&mut self) {
        self.queue.append(&mut self.previous_generation);
        std::mem::swap(&mut self.queue, &mut self.previous_generation);
        self.reset_at = None;
    }
    pub fn advance_to(&mut self, now: i64) {
        if self.reset_at.is_some_and(|at| at <= now) {
            self.reset_generation();
        }
    }
    fn check_send_next(&mut self) {
        if self.sent.len() >= MAX_WEB_FILE_QUERIES {
            return;
        }
        let entry = self
            .queue
            .pop_front()
            .or_else(|| self.previous_generation.pop_front());
        if let Some(e) = entry {
            self.next_reply += 1;
            let reply = self.next_reply;
            self.effects.push(WebEffect::Get {
                id: e.id,
                reply,
                url: e.url.clone(),
            });
            // C++ emplace does not replace an already-sent id, even though the
            // duplicate request has already been sent; preserve that ordering.
            self.sent.entry(e.id).or_insert(Sent {
                url: e.url,
                reply,
                stream: e.stream,
                data: Vec::new(),
                ready: 0,
                total: 0,
                redirects_left: MAX_HTTP_REDIRECTS,
            });
        }
    }
    fn remove_sent(&mut self, id: u64) {
        if let Some(s) = self.sent.remove(&id) {
            self.effects.push(WebEffect::Abort { reply: s.reply });
            self.check_send_next();
        }
    }
    fn matches(&self, id: u64, reply: u64) -> bool {
        self.sent.get(&id).is_some_and(|s| s.reply == reply)
    }
    pub fn failed(&mut self, id: u64, reply: u64) {
        if self.matches(id, reply) {
            self.remove_sent(id);
            self.effects.push(WebEffect::Update {
                id,
                data: Update::Error,
            });
        }
    }
    pub fn redirect(&mut self, id: u64, reply: u64, raw: &[u8]) {
        if !self.matches(id, reply) {
            return;
        }
        let s = &self.sent[&id];
        let utf8 = String::from_utf8_lossy(raw);
        let target = reqwest::Url::parse(&s.url)
            .ok()
            .and_then(|u| u.join(&utf8).ok());
        if raw.is_empty()
            || target
                .as_ref()
                .is_none_or(|u| !is_supported_web_url(u.as_str()))
        {
            self.failed(id, reply);
            return;
        }
        let s = self.sent.get_mut(&id).unwrap();
        let remaining = s.redirects_left;
        s.redirects_left -= 1;
        if remaining == 0 {
            self.failed(id, reply);
            return;
        }
        let url = target.unwrap().to_string();
        s.url = url.clone();
        self.next_reply += 1;
        s.reply = self.next_reply;
        self.effects.push(WebEffect::Abort { reply });
        self.effects.push(WebEffect::Get {
            id,
            reply: self.next_reply,
            url,
        });
    }
    /// HTTP adapter supplies normalized status/original-content-length and readAll
    /// bytes. Stale reply identities never mutate the replacement request.
    pub fn progress(&mut self, input: HttpProgress<'_>) {
        let HttpProgress {
            id,
            reply,
            status,
            ready,
            total,
            original_content_length,
            bytes,
            location,
        } = input;
        let total = if total <= 0 {
            original_content_length.unwrap_or(total)
        } else {
            total
        };
        match status {
            301 | 302 => {
                self.redirect(id, reply, location);
                return;
            }
            200 | 206 | 416 => {}
            _ => {
                self.failed(id, reply);
                return;
            }
        }
        if !self.matches(id, reply) {
            return;
        }
        let total = ready.max(total);
        if total <= 0 {
            self.failed(id, reply);
            return;
        }
        let s = self.sent.get_mut(&id).unwrap();
        s.ready = ready;
        s.total = total;
        if s.stream {
            if total > MAX_WEB_FILE {
                self.failed(id, reply);
                return;
            }
            self.effects.push(WebEffect::Update {
                id,
                data: Update::Progress {
                    ready,
                    total,
                    streamed: bytes.to_vec(),
                },
            });
            if ready >= total {
                self.finished(id, reply);
            }
        } else {
            if total > MAX_FILE_IN_MEMORY
                || (s.data.len() as i64).saturating_add(bytes.len() as i64) > MAX_FILE_IN_MEMORY
            {
                self.failed(id, reply);
                return;
            }
            s.data.extend_from_slice(bytes);
            if ready >= total {
                self.finished(id, reply);
            } else {
                self.effects.push(WebEffect::Update {
                    id,
                    data: Update::Progress {
                        ready,
                        total,
                        streamed: Vec::new(),
                    },
                });
            }
        }
    }
    fn finished(&mut self, id: u64, reply: u64) {
        if !self.matches(id, reply) {
            return;
        }
        let bytes = std::mem::take(&mut self.sent.get_mut(&id).unwrap().data);
        self.remove_sent(id);
        self.effects.push(WebEffect::Update {
            id,
            data: Update::Finished(bytes),
        });
    }
    pub fn clear(&mut self) {
        for (_, s) in std::mem::take(&mut self.sent) {
            self.effects.push(WebEffect::Abort { reply: s.reply });
        }
    }
    pub fn take_effects(&mut self) -> Vec<WebEffect> {
        std::mem::take(&mut self.effects)
    }
}
pub struct HttpProgress<'a> {
    pub id: u64,
    pub reply: u64,
    pub status: u16,
    pub ready: i64,
    pub total: i64,
    pub original_content_length: Option<i64>,
    pub bytes: &'a [u8],
    pub location: &'a [u8],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebLoaderEffect {
    Enqueue { url: String, stream: bool },
    Remove,
}
pub struct WebFileLoader {
    pub file: FileLoader,
    pub url: String,
    pub request_type: WebRequestType,
    ready: i64,
    streamed_offset: i64,
    active: bool,
    effects: Vec<WebLoaderEffect>,
}
impl WebFileLoader {
    pub fn new(mut file: FileLoader, url: String, request_type: WebRequestType) -> Self {
        file.set_reported_offset(0);
        Self {
            file,
            url,
            request_type,
            ready: 0,
            streamed_offset: 0,
            active: false,
            effects: Vec::new(),
        }
    }
    pub fn current_offset(&self) -> i64 {
        self.ready
    }
    pub fn stream_loading(&self) -> bool {
        self.file.is_file_only()
    }
    pub fn start_loading(&mut self) {
        if !self.file.finished() {
            self.active = true;
            self.effects.push(WebLoaderEffect::Enqueue {
                url: self.url.clone(),
                stream: self.stream_loading(),
            });
        }
    }
    pub fn cancel_request(&mut self) {
        if self.active {
            self.active = false;
            self.effects.push(WebLoaderEffect::Remove);
        }
    }
    pub fn close(&mut self) {
        if !self.file.finished() {
            self.cancel_request();
            self.file.cancel(FailureReason::NoFailure);
        }
    }
    pub fn take_effects(&mut self) -> Vec<WebLoaderEffect> {
        std::mem::take(&mut self.effects)
    }
    pub fn apply_update(&mut self, update: Update) {
        if !self.active {
            return;
        }
        match update {
            Update::Progress {
                ready,
                total,
                streamed,
            } => {
                self.file.set_sizes(total);
                self.ready = ready;
                self.file.set_reported_offset(ready);
                if !streamed.is_empty()
                    && !self.file.write_result_part(self.streamed_offset, &streamed)
                {
                    self.cancel_request();
                    self.file.cancel(FailureReason::OtherFailure);
                } else {
                    self.streamed_offset += streamed.len() as i64;
                    self.file.notify_about_progress();
                }
            }
            Update::Finished(bytes) => {
                self.cancel_request();
                if self.file.write_result_part(0, &bytes) {
                    self.file.finalize_result();
                }
            }
            Update::Error => {
                self.cancel_request();
                self.file.cancel(FailureReason::OtherFailure);
            }
        }
    }
}
/// Concrete HTTP client. Qt's NoLessSafe policy rejects non-HTTP destinations and
/// HTTPS downgrades in its automatic redirects (the explicit fallback is separate).
pub fn http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if !matches!(attempt.url().scheme(), "http" | "https")
                || attempt
                    .previous()
                    .last()
                    .is_some_and(|u| u.scheme() == "https" && attempt.url().scheme() == "http")
            {
                attempt.error("unsafe HTTP redirect")
            } else if attempt.previous().len() >= 50 {
                attempt.error("too many HTTP redirects")
            } else {
                attempt.follow()
            }
        }))
        .build()
}
/// Runs on the caller's existing runtime. Each synchronous callback must queue
/// input to its owner; cancellation is dropping this future by its real owner.
pub async fn http_get(
    client: &reqwest::Client,
    url: &str,
    mut progress: impl FnMut(u16, i64, i64, &[u8]),
) -> Result<(), reqwest::Error> {
    let mut reply = client.get(url).send().await?;
    let status = reply.status().as_u16();
    let total = reply
        .content_length()
        .and_then(|v| i64::try_from(v).ok())
        .unwrap_or(-1);
    let mut ready = 0;
    while let Some(bytes) = reply.chunk().await? {
        ready += bytes.len() as i64;
        progress(status, ready, total, &bytes);
    }
    // Qt emits a final progress even for empty responses.
    progress(status, ready, if total < 0 { ready } else { total }, &[]);
    Ok(())
}
