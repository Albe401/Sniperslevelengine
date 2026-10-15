//! CME transport boundary. Desktop activation stays disabled pending licensing.
//! Protocol sources and acceptance limits are recorded in docs/CME.md.
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Uat,
    Production,
}
impl Environment {
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "UAT" => Ok(Self::Uat),
            "PROD" => Ok(Self::Production),
            _ => Err(Error::Configuration),
        }
    }
    fn token(self) -> &'static str {
        match self {
            Self::Uat => "https://authnr.cmegroup.com/as/token.oauth2",
            Self::Production => "https://auth.cmegroup.com/as/token.oauth2",
        }
    }
    fn socket(self) -> &'static str {
        match self {
            Self::Uat => "wss://markets.api.uat.cmegroup.com/marketdatastream/v1",
            Self::Production => "wss://markets.api.cmegroup.com/marketdatastream/v1",
        }
    }
    fn reference(self) -> &'static str {
        match self {
            Self::Uat => "https://refdata.api.uat.cmegroup.com/refdata/v3",
            Self::Production => "https://refdata.api.cmegroup.com/refdata/v3",
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("CME pendiente de licencia")]
    PendingLicense,
    #[error("Configuración CME no válida")]
    Configuration,
    #[error("Autenticación CME no disponible")]
    Authentication,
    #[error("Conexión CME interrumpida")]
    Transport,
    #[error("Mensaje CME no válido")]
    Protocol,
    #[error("Suscripción CME rechazada")]
    Subscription,
}

// Never derive Debug or Serialize for credentials or bearer tokens.
pub struct Connector {
    environment: Environment,
    api_id: String,
    api_secret: String,
}
impl Connector {
    pub fn environment_name(&self) -> &'static str {
        match self.environment {
            Environment::Uat => "UAT",
            Environment::Production => "PROD",
        }
    }
    /// Pending license is checked before opening .env or reading credentials.
    /// The desktop does not expose an activation control while licensing is pending.
    pub fn load(licensed: bool) -> Result<Self, Error> {
        if !licensed {
            return Err(Error::PendingLicense);
        }
        let mut values = std::collections::BTreeMap::new();
        for name in ["CME_API_ID", "CME_API_SECRET", "CME_ENV"] {
            if let Ok(value) = std::env::var(name) {
                if !value.trim().is_empty() {
                    values.insert(name.to_owned(), value);
                }
            }
        }
        if !values.contains_key("CME_API_ID") || !values.contains_key("CME_API_SECRET") {
            let mut paths = Vec::new();
            if let Ok(path) = std::env::var("SNIPERS_ENV_FILE") {
                paths.push(std::path::PathBuf::from(path));
            } else {
                if let Ok(dir) = std::env::current_dir() {
                    crate::credentials::candidates(&dir, &mut paths);
                }
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(dir) = exe.parent() {
                        crate::credentials::candidates(dir, &mut paths);
                    }
                }
            }
            let path = paths
                .into_iter()
                .find(|p| p.is_file())
                .ok_or(Error::Configuration)?;
            for entry in dotenvy::from_path_iter(path).map_err(|_| Error::Configuration)? {
                let (name, value) = entry.map_err(|_| Error::Configuration)?;
                if ["CME_API_ID", "CME_API_SECRET", "CME_ENV"].contains(&name.as_str())
                    && !value.trim().is_empty()
                {
                    values.entry(name).or_insert(value);
                }
            }
        }
        let environment =
            Environment::parse(values.get("CME_ENV").map(String::as_str).unwrap_or("UAT"))?;
        Self::new(
            true,
            environment,
            values.remove("CME_API_ID").ok_or(Error::Configuration)?,
            values
                .remove("CME_API_SECRET")
                .ok_or(Error::Configuration)?,
        )
    }
    pub fn new(
        licensed: bool,
        environment: Environment,
        api_id: String,
        api_secret: String,
    ) -> Result<Self, Error> {
        if !licensed {
            return Err(Error::PendingLicense);
        }
        if api_id.trim().is_empty() || api_secret.trim().is_empty() {
            return Err(Error::Configuration);
        }
        Ok(Self {
            environment,
            api_id,
            api_secret,
        })
    }

    async fn authorize(&self) -> Result<(reqwest::Client, String), Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Transport)?;
        let response = client
            .post(self.environment.token())
            .basic_auth(&self.api_id, Some(&self.api_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(|_| Error::Authentication)?;
        if !response.status().is_success() {
            return Err(Error::Authentication);
        }
        let body = bounded_json(response)
            .await
            .map_err(|_| Error::Authentication)?;
        let token = body["access_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(Error::Authentication)?;
        Ok((client, token.to_owned()))
    }

    /// Completes every page before publishing metadata. URLs are constructed
    /// locally, never followed from a server-provided pagination link.
    pub async fn option_series(&self) -> Result<Vec<Value>, Error> {
        self.reference_rows("optionSeries", None).await
    }

    pub async fn series_instruments(&self, series_guid: &str) -> Result<Vec<Value>, Error> {
        if series_guid.is_empty()
            || series_guid.len() > 64
            || !series_guid
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(Error::Configuration);
        }
        let rows = self
            .reference_rows("instruments", Some(series_guid))
            .await?;
        if rows
            .iter()
            .any(|r| r["seriesGuid"].as_str() != Some(series_guid))
        {
            return Err(Error::Protocol);
        }
        Ok(rows)
    }

    async fn reference_rows(
        &self,
        collection: &str,
        series_guid: Option<&str>,
    ) -> Result<Vec<Value>, Error> {
        let (client, token) = self.authorize().await?;
        let mut result = Vec::new();
        let mut expected_total = None;
        let mut expected_pages = None;
        let mut identifiers = std::collections::BTreeSet::new();
        for page in 0..100_u64 {
            let mut query = vec![("page", page.to_string()), ("size", "500".to_owned())];
            if let Some(guid) = series_guid {
                query.push(("seriesGuid", guid.to_owned()));
            }
            let response = client
                .get(format!("{}/{collection}", self.environment.reference()))
                .bearer_auth(&token)
                .query(&query)
                .send()
                .await
                .map_err(|_| Error::Transport)?;
            if !response.status().is_success() {
                return Err(Error::Transport);
            }
            let value = bounded_json(response).await?;
            let (rows, pages, total) = reference_page(&value, page, collection)?;
            if expected_total.is_some_and(|previous| previous != total)
                || expected_pages.is_some_and(|previous| previous != pages)
            {
                return Err(Error::Protocol);
            }
            expected_total = Some(total);
            expected_pages = Some(pages);
            for row in rows {
                let field = if collection == "optionSeries" {
                    "seriesGuid"
                } else {
                    "guid"
                };
                let id = row[field]
                    .as_str()
                    .filter(|id| !id.is_empty() && id.len() <= 64)
                    .ok_or(Error::Protocol)?;
                if !identifiers.insert(id.to_owned()) {
                    return Err(Error::Protocol);
                }
            }
            result.extend(rows.iter().cloned());
            if result.len() > 50_000 || result.len() as u64 > total {
                return Err(Error::Protocol);
            }
            if page + 1 >= pages {
                return if result.len() as u64 == total {
                    Ok(result)
                } else {
                    Err(Error::Protocol)
                };
            }
        }
        Err(Error::Protocol)
    }

    /// One session. Caller must invalidate freshness on return and obtain a new
    /// token by calling this again when reconnecting. Bounded channel backpressure
    /// terminates the session instead of silently dropping market data.
    pub async fn run(
        &self,
        subscriptions: &[Product],
        output: tokio::sync::mpsc::Sender<Value>,
    ) -> Result<(), Error> {
        let subscription = subscription(subscriptions)?;
        let (_, token) = self.authorize().await?;
        let mut request = self
            .environment
            .socket()
            .into_client_request()
            .map_err(|_| Error::Configuration)?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {token}")
                .parse()
                .map_err(|_| Error::Authentication)?,
        );
        let (socket, _) = tokio::time::timeout(
            Duration::from_secs(20),
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| Error::Transport)?
        .map_err(|_| Error::Transport)?;
        run_session(socket, subscription, output).await
    }
}
async fn run_session<S>(
    mut socket: tokio_tungstenite::WebSocketStream<S>,
    subscription: String,
    output: tokio::sync::mpsc::Sender<Value>,
) -> Result<(), Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut gate = SessionGate::default();
    let login_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let deadline = if gate.authenticated {
            tokio::time::Instant::now() + Duration::from_secs(30)
        } else {
            login_deadline
        };
        let message = tokio::time::timeout_at(deadline, socket.next())
            .await
            .map_err(|_| Error::Transport)?
            .ok_or(Error::Transport)?
            .map_err(|_| Error::Transport)?;
        match message {
            Message::Text(text) => {
                let value = decode(&text)?;
                match gate.accept(&value)? {
                    Action::Subscribe => {
                        tokio::time::timeout(
                            Duration::from_secs(15),
                            socket.send(Message::Text(subscription.clone().into())),
                        )
                        .await
                        .map_err(|_| Error::Transport)?
                        .map_err(|_| Error::Transport)?;
                    }
                    Action::Ignore => {}
                    Action::Publish => {
                        crate::cme_data::normalize(&value)?;
                        output.try_send(value).map_err(|_| Error::Transport)?;
                    }
                }
            }
            Message::Ping(data) => {
                tokio::time::timeout(Duration::from_secs(15), socket.send(Message::Pong(data)))
                    .await
                    .map_err(|_| Error::Transport)?
                    .map_err(|_| Error::Transport)?
            }
            Message::Pong(_) => {}
            Message::Close(_) => return Err(Error::Transport),
            _ => return Err(Error::Protocol),
        }
    }
}

#[derive(Default)]
struct SessionGate {
    authenticated: bool,
    subscribed: std::collections::BTreeSet<String>,
    sequences: std::collections::BTreeMap<String, u64>,
}
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Subscribe,
    Ignore,
    Publish,
}
impl SessionGate {
    fn accept(&mut self, value: &Value) -> Result<Action, Error> {
        match value["header"]["messageType"]
            .as_str()
            .ok_or(Error::Protocol)?
        {
            "LOGINERR" => Err(Error::Authentication),
            "SUBSCRIPTION_ERROR" => Err(Error::Subscription),
            "HEARTBEAT" => Ok(Action::Ignore),
            "LOGINSTATUS" | "LOGIN_STATUS" => {
                if self.authenticated || value["payload"]["status"] != "authenticated" {
                    return Err(Error::Authentication);
                }
                self.authenticated = true;
                Ok(Action::Subscribe)
            }
            _ if !self.authenticated => Err(Error::Authentication),
            "SUBSCRIPTION_STATUS" => {
                let request = &value["header"]["requestId"];
                if (request.as_u64() != Some(1) && request.as_str() != Some("1"))
                    || value["payload"]["status"] != "SUBSCRIBED"
                {
                    return Err(Error::Subscription);
                }
                let topics = value["payload"]["subscriptionMessageTypes"]
                    .as_array()
                    .ok_or(Error::Subscription)?;
                if topics.is_empty()
                    || topics
                        .iter()
                        .any(|v| !matches!(v.as_str(), Some("TRD" | "TOB" | "STAT")))
                {
                    return Err(Error::Subscription);
                }
                self.subscribed
                    .extend(topics.iter().filter_map(Value::as_str).map(str::to_owned));
                Ok(Action::Ignore)
            }
            topic @ ("TRD" | "TOB" | "STAT") => {
                if self.subscribed.contains(topic) {
                    let raw = &value["header"]["sequenceNumber"];
                    let sequence = raw
                        .as_u64()
                        .or_else(|| raw.as_str()?.parse().ok())
                        .ok_or(Error::Protocol)?;
                    if self
                        .sequences
                        .get(topic)
                        .is_some_and(|last| last.checked_add(1) != Some(sequence))
                    {
                        return Err(Error::Protocol);
                    }
                    self.sequences.insert(topic.to_owned(), sequence);
                    Ok(Action::Publish)
                } else {
                    Err(Error::Subscription)
                }
            }
            _ => Ok(Action::Ignore),
        }
    }
}

async fn bounded_json(mut response: reqwest::Response) -> Result<Value, Error> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
        if bytes.len() + chunk.len() > 1_048_576 {
            return Err(Error::Protocol);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)
}

fn reference_page<'a>(
    value: &'a Value,
    page: u64,
    collection: &str,
) -> Result<(&'a Vec<Value>, u64, u64), Error> {
    let metadata = &value["_metadata"];
    let pages = metadata["totalPages"].as_u64().ok_or(Error::Protocol)?;
    let total = metadata["totalElements"].as_u64().ok_or(Error::Protocol)?;
    let rows = value["_embedded"][collection]
        .as_array()
        .ok_or(Error::Protocol)?;
    if metadata["number"].as_u64() != Some(page)
        || pages > 100
        || total > 50_000
        || rows.len() > 500
        || (pages == 0 && (total != 0 || !rows.is_empty()))
        || (pages > 0 && page >= pages)
        || (page + 1 < pages && rows.is_empty())
    {
        return Err(Error::Protocol);
    }
    Ok((rows, pages, total))
}

#[derive(Clone, Debug)]
pub struct Product {
    pub code: String,
    pub options: bool,
}
pub fn subscription(products: &[Product]) -> Result<String, Error> {
    if products.is_empty() || products.len() > 32 {
        return Err(Error::Configuration);
    }
    let mut rows = Vec::new();
    for product in products {
        if product.code.is_empty()
            || product.code.len() > 16
            || !product
                .code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(Error::Configuration);
        }
        let row = json!({"productType": if product.options {"OOF"} else {"FUT"}, "productCode":product.code});
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    Ok(
        json!({"header":{"messageType":"SUBSCRIBE","version":"1.0","requestId":1},
        "payload":{"subscriptionMessageTypes":["TRD","TOB","STAT"],"subscriptions":rows}})
        .to_string(),
    )
}

pub fn decode(text: &str) -> Result<Value, Error> {
    if text.len() > 1_048_576 {
        return Err(Error::Protocol);
    }
    let value: Value = serde_json::from_str(text).map_err(|_| Error::Protocol)?;
    if !value["header"]["messageType"].is_string() {
        return Err(Error::Protocol);
    }
    if value["payload"]
        .as_array()
        .is_some_and(|rows| rows.len() > 1024)
    {
        return Err(Error::Protocol);
    }
    Ok(value)
}

/// Missing values remain missing; zero and negative futures prices are valid.
pub fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_socket_authenticates_subscribes_and_delivers_only_market_data() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            ws.send(Message::Text(json!({"header":{"messageType":"LOGINSTATUS"},"payload":{"status":"authenticated"}}).to_string().into())).await.unwrap();
            let request = ws.next().await.unwrap().unwrap().into_text().unwrap();
            let request: Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["header"]["requestId"], 1);
            assert_eq!(request["payload"]["subscriptions"][0]["productCode"], "NQ");
            ws.send(Message::Text(json!({"header":{"messageType":"SUBSCRIPTION_STATUS","requestId":1},"payload":{"status":"SUBSCRIBED","subscriptionMessageTypes":["TRD"]}}).to_string().into())).await.unwrap();
            ws.send(Message::Text(json!({"header":{"messageType":"TRD","sequenceNumber":"1"},"payload":[{"instrument":{"id":"123","exchangeMic":"XCME","marketSegmentId":"1","symbol":"NQZ6","productCode":"NQ","productType":"FUT"},"lastUpdateTime":"2026-09-24T14:00:00Z","tradeSummary":{"tradePrice":"21000","tradeQty":"2"}}]}).to_string().into())).await.unwrap();
            ws.close(None).await.unwrap();
        });
        let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            run_session(
                socket,
                subscription(&[Product {
                    code: "NQ".into(),
                    options: false,
                }])
                .unwrap(),
                tx,
            ),
        )
        .await
        .unwrap();
        assert_eq!(result, Err(Error::Transport));
        let market = rx.try_recv().unwrap();
        assert_eq!(market["header"]["messageType"], "TRD");
        assert_eq!(crate::cme_data::normalize(&market).unwrap().len(), 1);
        assert!(rx.try_recv().is_err());
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
    }
    #[tokio::test]
    async fn local_socket_denied_login_never_sends_subscription() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            ws.send(Message::Text(
                json!({"header":{"messageType":"LOGINERR"}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            while let Some(Ok(frame)) = ws.next().await {
                assert!(!matches!(frame, Message::Text(_)));
            }
        });
        let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(5),
                run_session(socket, "must-not-send".into(), tx)
            )
            .await
            .unwrap(),
            Err(Error::Authentication)
        );
        assert!(rx.try_recv().is_err());
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
    }
    #[test]
    fn session_requires_login_and_subscription_ack_before_publishing() {
        let market = json!({"header":{"messageType":"TRD","sequenceNumber":"10"},"payload":[]});
        let login =
            json!({"header":{"messageType":"LOGINSTATUS"},"payload":{"status":"authenticated"}});
        let ack = json!({"header":{"messageType":"SUBSCRIPTION_STATUS","requestId":1},"payload":{"status":"SUBSCRIBED","subscriptionMessageTypes":["TRD","TOB","STAT"]}});
        let mut gate = SessionGate::default();
        assert_eq!(gate.accept(&market), Err(Error::Authentication));
        assert_eq!(gate.accept(&login), Ok(Action::Subscribe));
        assert_eq!(gate.accept(&market), Err(Error::Subscription));
        assert_eq!(gate.accept(&ack), Ok(Action::Ignore));
        assert_eq!(gate.accept(&market), Ok(Action::Publish));
        assert_eq!(gate.accept(&market), Err(Error::Protocol));
        let another_topic =
            json!({"header":{"messageType":"TOB","sequenceNumber":"100"},"payload":[]});
        assert_eq!(gate.accept(&another_topic), Ok(Action::Publish));
        let next = json!({"header":{"messageType":"TRD","sequenceNumber":"11"},"payload":[]});
        assert_eq!(gate.accept(&next), Ok(Action::Publish));
        let gap = json!({"header":{"messageType":"TRD","sequenceNumber":"13"},"payload":[]});
        assert_eq!(gate.accept(&gap), Err(Error::Protocol));
        assert_eq!(gate.accept(&login), Err(Error::Authentication));
        assert_eq!(
            SessionGate::default().accept(&ack),
            Err(Error::Authentication)
        );
        let failure = json!({"header":{"messageType":"LOGINERR"},"payload":{"message":"secret must not appear"}});
        assert_eq!(
            SessionGate::default()
                .accept(&failure)
                .unwrap_err()
                .to_string(),
            "Autenticación CME no disponible"
        );
    }
    #[test]
    fn reference_pages_require_consistent_bounded_metadata() {
        let mut page = json!({"_metadata":{"number":0,"totalPages":2,"totalElements":2},"_embedded":{"optionSeries":[{"globexSecurityId":"123"}]},"_links":{"next":{"href":"https://foreign.invalid/"}}});
        assert_eq!(reference_page(&page, 0, "optionSeries").unwrap().0.len(), 1);
        assert!(reference_page(&page, 1, "optionSeries").is_err());
        page["_embedded"]["optionSeries"] = json!([]);
        assert!(reference_page(&page, 0, "optionSeries").is_err());
        page["_metadata"] = json!({"number":0,"totalPages":0,"totalElements":0});
        assert!(reference_page(&page, 0, "optionSeries").is_ok());
        page["_metadata"]["totalPages"] = json!(101);
        assert!(reference_page(&page, 0, "optionSeries").is_err());
    }
    #[test]
    fn pending_license_precedes_credentials_and_cannot_create_transport() {
        assert!(matches!(Connector::load(false), Err(Error::PendingLicense)));
        assert!(matches!(
            Connector::new(false, Environment::Uat, String::new(), String::new()),
            Err(Error::PendingLicense)
        ));
        assert!(Environment::parse("production-typo").is_err());
        assert_ne!(Environment::Uat.socket(), Environment::Production.socket());
    }
    #[test]
    fn subscriptions_are_explicit_and_do_not_infer_option_product_codes() {
        let request: Value = serde_json::from_str(
            &subscription(&[Product {
                code: "NQ".into(),
                options: false,
            }])
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            request["payload"]["subscriptions"],
            json!([{"productCode":"NQ","productType":"FUT"}])
        );
        assert!(subscription(&[Product {
            code: "*".into(),
            options: true
        }])
        .is_err());
        assert!(subscription(&[]).is_err());
    }
    #[test]
    fn decoder_preserves_empty_book_and_absent_statistics() {
        let value = decode(r#"{"header":{"messageType":"TOB"},"payload":[{"askLevel":[],"bidLevel":[],"instrument":{"id":"123"}}]}"#).unwrap();
        assert_eq!(value["payload"][0]["askLevel"], json!([]));
        assert_eq!(number(&value["payload"][0]["openInterest"]), None);
        assert_eq!(number(&json!("-1.5")), Some(-1.5));
        for value in [json!("NaN"), json!("inf"), json!(true), Value::Null] {
            assert_eq!(number(&value), None);
        }
        assert!(decode("[]").is_err());
        assert!(decode(&" ".repeat(1_048_577)).is_err());
    }
}
