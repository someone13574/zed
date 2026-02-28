#[cfg(any(test, feature = "test-support"))]
#[path = "test.rs"]
pub mod test;
#[path = "telemetry_wasm.rs"]
pub mod telemetry;
#[path = "user.rs"]
pub mod user;
#[path = "zed_urls.rs"]
pub mod zed_urls;

use anyhow::{Result, anyhow};
use clock::SystemClock;
use cloud_api_client::CloudApiClient;
use cloud_api_client::websocket_protocol::MessageToClient;
use credentials_provider::CredentialsProvider;
use futures::{
    Future, FutureExt as _, Stream,
    channel::mpsc,
    future::BoxFuture,
};
use gpui::{App, AsyncApp, Entity, Global, Task, WeakEntity, actions};
use http_client::{HttpClientWithUrl, read_proxy_from_env};
use parking_lot::{Mutex, RwLock};
use postage::watch;
use release_channel::ReleaseChannel;
use rpc::proto::{AnyTypedEnvelope, EnvelopedMessage, PeerId, RequestMessage};
use serde::Deserialize;
use settings::{RegisterSetting, Settings, SettingsContent};
use std::{
    any::TypeId,
    future,
    marker::PhantomData,
    path::PathBuf,
    sync::{
        Arc, LazyLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use web_time::Instant;
use telemetry::Telemetry;
use thiserror::Error;
use url::Url;
use util::{ConnectionResult, ResultExt};

pub use rpc::*;
pub use telemetry_events::Event;
pub use user::*;

static ZED_SERVER_URL: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("ZED_SERVER_URL").ok());
static ZED_RPC_URL: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("ZED_RPC_URL").ok());

pub static IMPERSONATE_LOGIN: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("ZED_IMPERSONATE")
        .ok()
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
});

pub static USE_WEB_LOGIN: LazyLock<bool> = LazyLock::new(|| std::env::var("ZED_WEB_LOGIN").is_ok());

pub static ADMIN_API_TOKEN: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("ZED_ADMIN_API_TOKEN")
        .ok()
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
});

pub static ZED_APP_PATH: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| std::env::var("ZED_APP_PATH").ok().map(PathBuf::from));

pub static ZED_ALWAYS_ACTIVE: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ZED_ALWAYS_ACTIVE").is_ok_and(|value| !value.is_empty()));

pub const INITIAL_RECONNECTION_DELAY: Duration = Duration::from_millis(500);
pub const MAX_RECONNECTION_DELAY: Duration = Duration::from_secs(30);
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);

actions!(
    client,
    [
        /// Signs in to Zed account.
        SignIn,
        /// Signs out of Zed account.
        SignOut,
        /// Reconnects to the collaboration server.
        Reconnect
    ]
);

#[derive(Deserialize, RegisterSetting)]
pub struct ClientSettings {
    pub server_url: String,
}

impl Settings for ClientSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        if let Some(server_url) = &*ZED_SERVER_URL {
            return Self {
                server_url: server_url.clone(),
            };
        }

        Self {
            server_url: content.server_url.clone().unwrap_or_else(|| "https://zed.dev".to_string()),
        }
    }
}

#[derive(Deserialize, Default, RegisterSetting)]
pub struct ProxySettings {
    pub proxy: Option<String>,
}

impl ProxySettings {
    pub fn proxy_url(&self) -> Option<Url> {
        self.proxy
            .as_deref()
            .map(str::trim)
            .filter(|input| !input.is_empty())
            .and_then(|input| {
                input
                    .parse::<Url>()
                    .inspect_err(|error| log::error!("Error parsing proxy settings: {error}"))
                    .ok()
            })
            .or_else(read_proxy_from_env)
    }
}

impl Settings for ProxySettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        Self {
            proxy: content
                .proxy
                .as_deref()
                .map(str::trim)
                .filter(|proxy| !proxy.is_empty())
                .map(ToOwned::to_owned),
        }
    }
}

pub fn init(_client: &Arc<Client>, _cx: &mut App) {}

pub type MessageToClientHandler = Box<dyn Fn(&MessageToClient, &mut App) + Send + Sync + 'static>;

struct GlobalClient(Arc<Client>);

impl Global for GlobalClient {}

pub struct Client {
    id: AtomicU64,
    peer: Arc<Peer>,
    http: Arc<HttpClientWithUrl>,
    cloud_client: Arc<CloudApiClient>,
    telemetry: Arc<Telemetry>,
    credentials_provider: ClientCredentialsProvider,
    state: RwLock<ClientState>,
    handler_set: Mutex<ProtoMessageHandlerSet>,
    message_to_client_handlers: Mutex<Vec<MessageToClientHandler>>,
    sign_out_tx: Mutex<Option<mpsc::UnboundedSender<()>>>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    authenticate:
        RwLock<Option<Box<dyn 'static + Send + Sync + Fn(&AsyncApp) -> Task<Result<Credentials>>>>>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    establish_connection: RwLock<
        Option<
            Box<
                dyn 'static
                    + Send
                    + Sync
                    + Fn(
                        &Credentials,
                        &AsyncApp,
                    ) -> Task<Result<Connection, EstablishConnectionError>>,
            >,
        >,
    >,

    #[cfg(any(test, feature = "test-support"))]
    rpc_url: RwLock<Option<Url>>,
}

#[derive(Error, Debug)]
pub enum EstablishConnectionError {
    #[error("upgrade required")]
    UpgradeRequired,
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl EstablishConnectionError {
    pub fn other(error: impl Into<anyhow::Error> + Send + Sync) -> Self {
        Self::Other(error.into())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    SignedOut,
    UpgradeRequired,
    Authenticating,
    Authenticated,
    AuthenticationError,
    Connecting,
    ConnectionError,
    Connected {
        peer_id: PeerId,
        connection_id: ConnectionId,
    },
    ConnectionLost,
    Reauthenticating,
    Reauthenticated,
    Reconnecting,
    ReconnectionError {
        next_reconnection: Instant,
    },
}

impl Status {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn was_connected(&self) -> bool {
        matches!(
            self,
            Self::ConnectionLost
                | Self::Reauthenticating
                | Self::Reauthenticated
                | Self::Reconnecting
        )
    }

    pub fn is_or_was_connected(&self) -> bool {
        self.is_connected() || self.was_connected()
    }

    pub fn is_signing_in(&self) -> bool {
        matches!(
            self,
            Self::Authenticating | Self::Reauthenticating | Self::Connecting | Self::Reconnecting
        )
    }

    pub fn is_signed_out(&self) -> bool {
        matches!(self, Self::SignedOut | Self::UpgradeRequired)
    }
}

struct ClientState {
    credentials: Option<Credentials>,
    status: (watch::Sender<Status>, watch::Receiver<Status>),
    _reconnect_task: Option<Task<()>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    pub user_id: u64,
    pub access_token: String,
}

impl Credentials {
    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.user_id, self.access_token)
    }
}

pub struct ClientCredentialsProvider {
    provider: Arc<dyn CredentialsProvider>,
}

impl ClientCredentialsProvider {
    pub fn new(cx: &App) -> Self {
        Self {
            provider: <dyn CredentialsProvider>::global(cx),
        }
    }

    fn server_url(&self, cx: &AsyncApp) -> Result<String> {
        Ok(cx.update(|cx| ClientSettings::get_global(cx).server_url.clone()))
    }

    fn read_credentials<'a>(
        &'a self,
        cx: &'a AsyncApp,
    ) -> futures::future::LocalBoxFuture<'a, Option<Credentials>> {
        async move {
            if IMPERSONATE_LOGIN.is_some() {
                return None;
            }

            let server_url = self.server_url(cx).ok()?;
            let (user_id, access_token) = self
                .provider
                .read_credentials(&server_url, cx)
                .await
                .log_err()
                .flatten()?;

            Some(Credentials {
                user_id: user_id.parse().ok()?,
                access_token: String::from_utf8(access_token).ok()?,
            })
        }
        .boxed_local()
    }

    fn write_credentials<'a>(
        &'a self,
        user_id: u64,
        access_token: String,
        cx: &'a AsyncApp,
    ) -> futures::future::LocalBoxFuture<'a, Result<()>> {
        async move {
            let server_url = self.server_url(cx)?;
            self.provider
                .write_credentials(
                    &server_url,
                    &user_id.to_string(),
                    access_token.as_bytes(),
                    cx,
                )
                .await
        }
        .boxed_local()
    }

    fn delete_credentials<'a>(
        &'a self,
        cx: &'a AsyncApp,
    ) -> futures::future::LocalBoxFuture<'a, Result<()>> {
        async move {
            let server_url = self.server_url(cx)?;
            self.provider.delete_credentials(&server_url, cx).await
        }
        .boxed_local()
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            credentials: None,
            status: watch::channel_with(Status::SignedOut),
            _reconnect_task: None,
        }
    }
}

pub enum Subscription {
    Entity {
        client: Weak<Client>,
        id: (TypeId, u64),
    },
    Message {
        client: Weak<Client>,
        id: TypeId,
    },
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self {
            Subscription::Entity { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.handler_set.lock();
                    let _ = state.entities_by_type_and_remote_id.remove(id);
                }
            }
            Subscription::Message { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.handler_set.lock();
                    let _ = state.entity_types_by_message_type.remove(id);
                    let _ = state.message_handlers.remove(id);
                }
            }
        }
    }
}

pub struct PendingEntitySubscription<T: 'static> {
    client: Arc<Client>,
    remote_id: u64,
    _entity_type: PhantomData<T>,
    consumed: bool,
}

impl<T: 'static> PendingEntitySubscription<T> {
    pub fn set_entity(mut self, _entity: &Entity<T>, _cx: &AsyncApp) -> Subscription {
        self.consumed = true;
        Subscription::Entity {
            client: Arc::downgrade(&self.client),
            id: (TypeId::of::<T>(), self.remote_id),
        }
    }
}

impl<T: 'static> Drop for PendingEntitySubscription<T> {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self.client.handler_set.lock();
            let _ = state
                .entities_by_type_and_remote_id
                .remove(&(TypeId::of::<T>(), self.remote_id));
        }
    }
}

#[derive(Copy, Clone, Deserialize, Debug, RegisterSetting)]
pub struct TelemetrySettings {
    pub diagnostics: bool,
    pub metrics: bool,
}

impl settings::Settings for TelemetrySettings {
    fn from_settings(content: &SettingsContent) -> Self {
        Self {
            diagnostics: content.telemetry.as_ref().is_some_and(|value| value.diagnostics.unwrap_or(false)),
            metrics: content.telemetry.as_ref().is_some_and(|value| value.metrics.unwrap_or(false)),
        }
    }
}

impl Client {
    pub fn new(clock: Arc<dyn SystemClock>, http: Arc<HttpClientWithUrl>, cx: &mut App) -> Arc<Self> {
        Arc::new(Self {
            id: AtomicU64::new(0),
            peer: Peer::new(0),
            telemetry: Telemetry::new(clock, http.clone(), cx),
            cloud_client: Arc::new(CloudApiClient::new(http.clone())),
            http,
            credentials_provider: ClientCredentialsProvider::new(cx),
            state: Default::default(),
            handler_set: Default::default(),
            message_to_client_handlers: Mutex::new(Vec::new()),
            sign_out_tx: Mutex::new(None),

            #[cfg(any(test, feature = "test-support"))]
            authenticate: Default::default(),
            #[cfg(any(test, feature = "test-support"))]
            establish_connection: Default::default(),
            #[cfg(any(test, feature = "test-support"))]
            rpc_url: RwLock::default(),
        })
    }

    pub fn production(cx: &mut App) -> Arc<Self> {
        let clock = Arc::new(clock::RealSystemClock);
        let http = Arc::new(HttpClientWithUrl::new_url(
            cx.http_client(),
            &ClientSettings::get_global(cx).server_url,
            cx.http_client().proxy().cloned(),
        ));
        Self::new(clock, http, cx)
    }

    pub fn id(&self) -> u64 {
        self.id.load(Ordering::SeqCst)
    }

    pub fn http_client(&self) -> Arc<HttpClientWithUrl> {
        self.http.clone()
    }

    pub fn cloud_client(&self) -> Arc<CloudApiClient> {
        self.cloud_client.clone()
    }

    pub fn set_id(&self, id: u64) -> &Self {
        self.id.store(id, Ordering::SeqCst);
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn teardown(&self) {
        let mut state = self.state.write();
        state._reconnect_task.take();
        self.handler_set.lock().clear();
        self.peer.teardown();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn override_authenticate<F>(&self, authenticate: F) -> &Self
    where
        F: 'static + Send + Sync + Fn(&AsyncApp) -> Task<Result<Credentials>>,
    {
        *self.authenticate.write() = Some(Box::new(authenticate));
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn override_establish_connection<F>(&self, connect: F) -> &Self
    where
        F: 'static
            + Send
            + Sync
            + Fn(&Credentials, &AsyncApp) -> Task<Result<Connection, EstablishConnectionError>>,
    {
        *self.establish_connection.write() = Some(Box::new(connect));
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn override_rpc_url(&self, url: Url) -> &Self {
        *self.rpc_url.write() = Some(url);
        self
    }

    pub fn global(cx: &App) -> Arc<Self> {
        cx.global::<GlobalClient>().0.clone()
    }

    pub fn set_global(client: Arc<Client>, cx: &mut App) {
        cx.set_global(GlobalClient(client));
    }

    pub fn user_id(&self) -> Option<u64> {
        self.state
            .read()
            .credentials
            .as_ref()
            .map(|credentials| credentials.user_id)
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        if let Status::Connected { peer_id, .. } = &*self.status().borrow() {
            Some(*peer_id)
        } else {
            None
        }
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.state.read().status.1.clone()
    }

    fn set_status(self: &Arc<Self>, status: Status, _cx: &AsyncApp) {
        let mut state = self.state.write();
        *state.status.0.borrow_mut() = status;
        if matches!(status, Status::Connected { .. }) {
            state._reconnect_task = None;
        }
    }

    pub fn subscribe_to_entity<T>(self: &Arc<Self>, remote_id: u64) -> Result<PendingEntitySubscription<T>>
    where
        T: 'static,
    {
        let id = (TypeId::of::<T>(), remote_id);

        let mut state = self.handler_set.lock();
        anyhow::ensure!(
            !state.entities_by_type_and_remote_id.contains_key(&id),
            "already subscribed to entity"
        );

        state
            .entities_by_type_and_remote_id
            .insert(id, EntityMessageSubscriber::Pending(Default::default()));

        Ok(PendingEntitySubscription {
            client: self.clone(),
            remote_id,
            consumed: false,
            _entity_type: PhantomData,
        })
    }

    #[track_caller]
    pub fn add_message_handler<M, E, H, F>(
        self: &Arc<Self>,
        _entity: WeakEntity<E>,
        _handler: H,
    ) -> Subscription
    where
        M: EnvelopedMessage,
        E: 'static,
        H: 'static + Sync + Fn(Entity<E>, TypedEnvelope<M>, AsyncApp) -> F + Send + Sync,
        F: 'static + Future<Output = Result<()>>,
    {
        Subscription::Message {
            client: Arc::downgrade(self),
            id: TypeId::of::<M>(),
        }
    }

    pub fn add_request_handler<M, E, H, F>(
        self: &Arc<Self>,
        _entity: WeakEntity<E>,
        _handler: H,
    ) -> Subscription
    where
        M: RequestMessage,
        E: 'static,
        H: 'static + Sync + Fn(Entity<E>, TypedEnvelope<M>, AsyncApp) -> F + Send + Sync,
        F: 'static + Future<Output = Result<M::Response>>,
    {
        Subscription::Message {
            client: Arc::downgrade(self),
            id: TypeId::of::<M>(),
        }
    }

    pub async fn has_credentials(&self, cx: &AsyncApp) -> bool {
        self.credentials_provider.read_credentials(cx).await.is_some()
    }

    pub async fn sign_in(self: &Arc<Self>, _try_provider: bool, _cx: &AsyncApp) -> Result<Credentials> {
        Err(anyhow!("client sign-in is unavailable on wasm"))
    }

    pub async fn sign_in_with_optional_connect(
        self: &Arc<Self>,
        _try_provider: bool,
        _cx: &AsyncApp,
    ) -> Result<()> {
        Ok(())
    }

    pub async fn connect(self: &Arc<Self>, _try_provider: bool, _cx: &AsyncApp) -> ConnectionResult<()> {
        ConnectionResult::Result(Err(anyhow!("client connection is unavailable on wasm")))
    }

    pub fn authenticate_with_browser(self: &Arc<Self>, _cx: &AsyncApp) -> Task<Result<Credentials>> {
        Task::ready(Err(anyhow!("browser auth is unavailable on wasm")))
    }

    pub async fn sign_out(self: &Arc<Self>, cx: &AsyncApp) {
        self.state.write().credentials = None;
        self.cloud_client.clear_credentials();
        self.disconnect(cx);

        if self.has_credentials(cx).await {
            self.credentials_provider.delete_credentials(cx).await.log_err();
        }
    }

    pub fn request_sign_out(&self) {
        if let Some(sign_out_tx) = self.sign_out_tx.lock().clone() {
            sign_out_tx.unbounded_send(()).ok();
        }
    }

    pub fn disconnect(self: &Arc<Self>, cx: &AsyncApp) {
        self.peer.teardown();
        self.set_status(Status::SignedOut, cx);
    }

    pub fn reconnect(self: &Arc<Self>, cx: &AsyncApp) {
        self.peer.teardown();
        self.set_status(Status::ConnectionLost, cx);
    }

    pub fn send<T: EnvelopedMessage>(&self, _message: T) -> Result<()> {
        Err(anyhow!("rpc send is unavailable on wasm"))
    }

    pub fn request<T: RequestMessage>(
        &self,
        _request: T,
    ) -> impl Future<Output = Result<T::Response>> + use<T> {
        future::ready(Err(anyhow!("rpc request is unavailable on wasm")))
    }

    pub fn request_stream<T: RequestMessage>(
        &self,
        _request: T,
    ) -> impl Future<Output = Result<impl Stream<Item = Result<T::Response>>>> {
        future::ready(Ok(futures::stream::empty()))
    }

    pub fn request_envelope<T: RequestMessage>(
        &self,
        _request: T,
    ) -> impl Future<Output = Result<TypedEnvelope<T::Response>>> + use<T> {
        future::ready(Err(anyhow!("rpc request is unavailable on wasm")))
    }

    pub fn request_dynamic(
        &self,
        _envelope: proto::Envelope,
        _request_type: &'static str,
    ) -> impl Future<Output = Result<proto::Envelope>> + use<> {
        future::ready(Err(anyhow!("rpc request is unavailable on wasm")))
    }

    pub fn add_message_to_client_handler(
        self: &Arc<Client>,
        handler: impl Fn(&MessageToClient, &mut App) + Send + Sync + 'static,
    ) {
        self.message_to_client_handlers.lock().push(Box::new(handler));
    }

    pub fn telemetry(&self) -> &Arc<Telemetry> {
        &self.telemetry
    }
}

impl ProtoClient for Client {
    fn request(
        &self,
        _envelope: proto::Envelope,
        _request_type: &'static str,
    ) -> BoxFuture<'static, Result<proto::Envelope>> {
        future::ready(Err(anyhow!("rpc request is unavailable on wasm"))).boxed()
    }

    fn send(&self, _envelope: proto::Envelope, _message_type: &'static str) -> Result<()> {
        Err(anyhow!("rpc send is unavailable on wasm"))
    }

    fn send_response(&self, _envelope: proto::Envelope, _message_type: &'static str) -> Result<()> {
        Err(anyhow!("rpc send is unavailable on wasm"))
    }

    fn message_handler_set(&self) -> &Mutex<ProtoMessageHandlerSet> {
        &self.handler_set
    }

    fn is_via_collab(&self) -> bool {
        false
    }

    fn has_wsl_interop(&self) -> bool {
        false
    }
}

pub const ZED_URL_SCHEME: &str = "zed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZedLink {
    Channel { channel_id: u64 },
    ChannelNotes {
        channel_id: u64,
        heading: Option<String>,
    },
}

pub fn parse_zed_link(link: &str, cx: &App) -> Option<ZedLink> {
    let server_url = &ClientSettings::get_global(cx).server_url;
    let path = link
        .strip_prefix(server_url)
        .and_then(|result| result.strip_prefix('/'))
        .or_else(|| {
            link.strip_prefix(ZED_URL_SCHEME)
                .and_then(|result| result.strip_prefix("://"))
        })?;

    let mut parts = path.split('/');

    if parts.next() != Some("channel") {
        return None;
    }

    let slug = parts.next()?;
    let id_str = slug.split('-').next_back()?;
    let channel_id = id_str.parse::<u64>().ok()?;

    let Some(next) = parts.next() else {
        return Some(ZedLink::Channel { channel_id });
    };

    if let Some(heading) = next.strip_prefix("notes#") {
        return Some(ZedLink::ChannelNotes {
            channel_id,
            heading: Some(heading.to_string()),
        });
    }

    if next == "notes" {
        return Some(ZedLink::ChannelNotes {
            channel_id,
            heading: None,
        });
    }

    None
}
