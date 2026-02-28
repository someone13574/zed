use anyhow::Result;
use clock::SystemClock;
use futures::channel::mpsc;
use gpui::{App, BackgroundExecutor, Task};
use http_client::HttpClientWithUrl;
use std::path::PathBuf;
use std::sync::Arc;
use telemetry_events::{AssistantEventData, EventWrapper};

pub struct TelemetrySubscription {
    pub historical_events: Result<HistoricalEvents>,
    pub queued_events: Vec<EventWrapper>,
    pub live_events: mpsc::UnboundedReceiver<EventWrapper>,
}

pub struct HistoricalEvents {
    pub events: Vec<EventWrapper>,
    pub parse_error_count: usize,
}

pub struct Telemetry {
    _clock: Arc<dyn SystemClock>,
    _http_client: Arc<HttpClientWithUrl>,
    _executor: BackgroundExecutor,
}

pub static MINIDUMP_ENDPOINT: std::sync::LazyLock<Option<String>> = std::sync::LazyLock::new(|| {
    option_env!("ZED_MINIDUMP_ENDPOINT")
        .map(str::to_string)
        .or_else(|| std::env::var("ZED_MINIDUMP_ENDPOINT").ok())
});

pub fn os_name() -> String {
    "Web".to_string()
}

pub fn os_version() -> String {
    "unknown".to_string()
}

impl Telemetry {
    pub fn new(clock: Arc<dyn SystemClock>, client: Arc<HttpClientWithUrl>, cx: &mut App) -> Arc<Self> {
        Arc::new(Self {
            _clock: clock,
            _http_client: client,
            _executor: cx.background_executor().clone(),
        })
    }

    pub fn log_file_path() -> PathBuf {
        PathBuf::from("telemetry.log")
    }

    pub async fn subscribe_with_history<T>(self: &Arc<Self>, _fs: T) -> TelemetrySubscription {
        let (_tx, rx) = mpsc::unbounded();
        TelemetrySubscription {
            historical_events: Ok(HistoricalEvents {
                events: Vec::new(),
                parse_error_count: 0,
            }),
            queued_events: Vec::new(),
            live_events: rx,
        }
    }

    pub fn has_checksum_seed(&self) -> bool {
        false
    }

    pub fn start(
        self: &Arc<Self>,
        _system_id: Option<String>,
        _installation_id: Option<String>,
        _session_id: String,
        _cx: &mut App,
    ) {
    }

    pub fn metrics_enabled(self: &Arc<Self>) -> bool {
        false
    }

    pub fn diagnostics_enabled(self: &Arc<Self>) -> bool {
        false
    }

    pub fn set_authenticated_user_info(
        self: &Arc<Self>,
        _metrics_id: Option<String>,
        _is_staff: bool,
    ) {
    }

    pub fn report_assistant_event(self: &Arc<Self>, _event: AssistantEventData) {}

    pub fn log_edit_event(self: &Arc<Self>, _environment: &'static str, _is_via_ssh: bool) {}

    pub fn report_discovered_project_type_events<T, U>(
        self: &Arc<Self>,
        _worktree_id: T,
        _updated_entries_set: U,
    ) {
    }

    pub fn metrics_id(self: &Arc<Self>) -> Option<Arc<str>> {
        None
    }

    pub fn system_id(self: &Arc<Self>) -> Option<Arc<str>> {
        None
    }

    pub fn installation_id(self: &Arc<Self>) -> Option<Arc<str>> {
        None
    }

    pub fn is_staff(self: &Arc<Self>) -> Option<bool> {
        None
    }

    pub async fn flush_events_inner(self: &Arc<Self>) -> Result<()> {
        Ok(())
    }

    pub fn flush_events(self: &Arc<Self>) -> Task<()> {
        Task::ready(())
    }
}

pub fn calculate_json_checksum(_json: &impl AsRef<[u8]>) -> Option<String> {
    None
}
