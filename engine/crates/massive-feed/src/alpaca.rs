//! Alpaca PAPER only. No configurable trading host and no automatic signal execution.
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Method,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

pub const PAPER_URL: &str = "https://paper-api.alpaca.markets/v2";
const FILE: &str = "data/alpaca-paper.credentials.dpapi";

// Deliberately no Debug or Serialize: secrets never cross IPC or diagnostics.
#[derive(Deserialize)]
pub struct Credentials {
    key: String,
    secret: String,
}
impl Credentials {
    pub fn new(key: String, secret: String) -> Result<Self, String> {
        if !key.starts_with("PK")
            || key.len() < 16
            || secret.len() < 20
            || !key.bytes().all(|c| c.is_ascii_alphanumeric())
            || !secret.bytes().all(|c| c.is_ascii_alphanumeric())
        {
            return Err("Se requieren claves válidas de Alpaca Paper (PK).".into());
        }
        Ok(Self { key, secret })
    }
    pub fn load() -> Result<Self, String> {
        let path = credential_path()?;
        let encrypted = std::fs::read_to_string(path)
            .map_err(|_| "Faltan las credenciales locales de Alpaca Paper")?;
        let plain = dpapi(&encrypted, false)?;
        let parsed: Self =
            serde_json::from_str(&plain).map_err(|_| "Credenciales guardadas no válidas")?;
        Self::new(parsed.key, parsed.secret)
    }
    pub fn save(&self) -> Result<(), String> {
        let plain = json!({"key": self.key, "secret": self.secret}).to_string();
        let encrypted = dpapi(&plain, true)?;
        let path = credential_path()?;
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|_| "No se pudo crear el almacén local")?;
        std::fs::write(path, encrypted)
            .map_err(|_| "No se pudieron guardar las credenciales".into())
    }
}
fn credential_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("SNIPERS_ALPACA_CREDENTIAL_FILE") {
        return Ok(path.into());
    }
    for start in [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_owned())),
    ]
    .into_iter()
    .flatten()
    {
        for dir in start.ancestors().take(8) {
            if dir.join("snipers-rust/Cargo.toml").is_file() {
                return Ok(dir.join(FILE));
            }
        }
    }
    Err("Configura SNIPERS_ALPACA_CREDENTIAL_FILE para localizar las credenciales".into())
}
#[cfg(windows)]
fn dpapi(input: &str, protect: bool) -> Result<String, String> {
    use std::{
        io::Write,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    let action = if protect {
        "[Convert]::ToBase64String([Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes($v),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser))"
    } else {
        "[Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($v),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser))"
    };
    let script = format!("$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Security; $v=[Console]::In.ReadToEnd(); {action}");
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "No se pudo abrir el almacén cifrado de Windows")?;
    child
        .stdin
        .take()
        .ok_or("Entrada segura no disponible")?
        .write_all(input.as_bytes())
        .map_err(|_| "No se pudo acceder al almacén cifrado")?;
    let output = child
        .wait_with_output()
        .map_err(|_| "No se pudo completar el cifrado")?;
    if !output.status.success() {
        return Err("Windows no pudo descifrar las claves para este usuario".into());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "Almacén cifrado no válido".into())
}
#[cfg(not(windows))]
fn dpapi(_: &str, _: bool) -> Result<String, String> {
    Err("Este almacén requiere Windows DPAPI".into())
}

/// Whether POST /orders was attempted. Only a preflight failure is safe to retire.
pub struct SubmissionError {
    pub message: String,
    pub attempted: bool,
}
pub struct PaperClient {
    http: Client,
    #[cfg(test)]
    test_base: Option<String>,
}
impl PaperClient {
    pub fn new(credentials: Credentials) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("APCA-API-KEY-ID", credentials.key),
            ("APCA-API-SECRET-KEY", credentials.secret),
        ] {
            let mut h = HeaderValue::from_str(&value).map_err(|_| "Credenciales no válidas")?;
            h.set_sensitive(true);
            headers.insert(name, h);
        }
        let http = Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| "No se pudo iniciar Alpaca Paper")?;
        Ok(Self {
            http,
            #[cfg(test)]
            test_base: None,
        })
    }
    pub fn load() -> Result<Self, String> {
        Self::new(Credentials::load()?)
    }
    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let base = PAPER_URL;
        #[cfg(test)]
        let base = self.test_base.as_deref().unwrap_or(base);
        let mut request = self
            .http
            .request(method, format!("{base}{path}"))
            .query(query);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|_| "Alpaca Paper no respondió. Comprueba la conexión; antes de repetir una orden consulta su client_order_id.")?;
        let status = response.status();
        if !status.is_success() {
            return Err(http_error(status.as_u16()));
        }
        if status.as_u16() == 204 {
            return Ok(Value::Null);
        }
        response
            .json()
            .await
            .map_err(|_| "Respuesta de Alpaca Paper no válida".into())
    }
    pub async fn account(&self) -> Result<Value, String> {
        self.request(Method::GET, "/account", &[], None).await
    }
    pub async fn clock(&self) -> Result<Value, String> {
        self.request(Method::GET, "/clock", &[], None).await
    }
    pub async fn positions(&self) -> Result<Value, String> {
        self.request(Method::GET, "/positions", &[], None).await
    }
    pub async fn open_orders(&self) -> Result<Value, String> {
        self.request(
            Method::GET,
            "/orders",
            &[("status", "open"), ("limit", "500")],
            None,
        )
        .await
    }
    pub async fn snapshot(&self) -> Result<Value, String> {
        let (account, positions, orders, clock) = tokio::try_join!(
            self.account(),
            self.request(Method::GET, "/positions", &[], None),
            self.request(
                Method::GET,
                "/orders",
                &[("status", "all"), ("limit", "100"), ("direction", "desc")],
                None
            ),
            self.request(Method::GET, "/clock", &[], None)
        )?;
        Ok(
            json!({"environment":"paper","automatic_execution":false,"account":account,"positions":positions,"orders":orders,"clock":clock,"orders_limit":100}),
        )
    }
    pub async fn contracts(
        &self,
        underlying: &str,
        expiration: &str,
        page: Option<&str>,
    ) -> Result<Value, String> {
        if !underlying.bytes().all(|b| b.is_ascii_uppercase())
            || underlying.is_empty()
            || underlying.len() > 6
            || !valid_date(expiration)
        {
            return Err("Activo o vencimiento no válido".into());
        }
        let mut q = vec![
            ("underlying_symbols", underlying),
            ("expiration_date", expiration),
            ("status", "active"),
            ("limit", "100"),
        ];
        if let Some(page) = page {
            q.push(("page_token", page));
        }
        self.request(Method::GET, "/options/contracts", &q, None)
            .await
    }
    pub async fn order(&self, client_id: &str) -> Result<Value, String> {
        validate_id(client_id)?;
        self.request(
            Method::GET,
            "/orders:by_client_order_id",
            &[("client_order_id", client_id)],
            None,
        )
        .await
    }
    /// Explicit OPRA only. No silent downgrade to indicative/modified quotes.
    pub async fn option_quote(&self, symbol: &str) -> Result<Value, String> {
        OrderIntent {
            symbol: symbol.into(),
            qty: 1,
            limit_price: "1".into(),
            side: OrderSide::Buy,
            client_order_id: "snipers-quote-validation".into(),
        }
        .validate()?;
        let response = self
            .http
            .get("https://data.alpaca.markets/v1beta1/options/quotes/latest")
            .query(&[("symbols", symbol), ("feed", "opra")])
            .send()
            .await
            .map_err(|_| "No se pudo consultar la cotización OPRA")?;
        if !response.status().is_success() {
            return Err(http_error(response.status().as_u16()));
        }
        let data: Value = response
            .json()
            .await
            .map_err(|_| "Cotización OPRA no válida")?;
        Ok(json!({"feed":"opra","data":data}))
    }
    /// Explicit intent from the bot controller. Never called by the current local simulator.
    /// No automatic POST retries: reconcile client_order_id after uncertain network outcomes.
    pub async fn submit(&self, intent: OrderIntent) -> Result<Value, String> {
        self.submit_fresh(intent, None).await
    }
    pub async fn submit_fresh(
        &self,
        intent: OrderIntent,
        quote_ms: Option<u64>,
    ) -> Result<Value, String> {
        self.submit_outcome(intent, quote_ms)
            .await
            .map_err(|e| e.message)
    }
    pub async fn submit_outcome(
        &self,
        intent: OrderIntent,
        quote_ms: Option<u64>,
    ) -> Result<Value, SubmissionError> {
        let preflight = async {
            intent.validate()?;
            let account = self.account().await?;
            if account["status"] != "ACTIVE"
                || account["trading_blocked"] != false
                || account["account_blocked"] != false
                || (intent.side == OrderSide::Buy
                    && account["options_trading_level"].as_u64().unwrap_or(0) < 2)
            {
                return Err("La cuenta Paper no permite operar opciones en este momento".into());
            }
            let clock = self.request(Method::GET, "/clock", &[], None).await?;
            if clock["is_open"] != true {
                return Err(
                    "Mercado cerrado: no se encolan órdenes del bot para otra sesión".into(),
                );
            }
            let contract = self
                .request(
                    Method::GET,
                    &format!("/options/contracts/{}", intent.symbol),
                    &[],
                    None,
                )
                .await?;
            if contract["tradable"] != true || contract["status"] != "active" {
                return Err("El contrato no está disponible para operar".into());
            }
            // Closing is long-only: never create uncovered short option exposure.
            if intent.side == OrderSide::Sell {
                let positions = self.request(Method::GET, "/positions", &[], None).await?;
                let held = positions
                    .as_array()
                    .and_then(|p| {
                        p.iter()
                            .find(|p| p["symbol"] == intent.symbol && p["side"] == "long")
                    })
                    .and_then(|p| p["qty_available"].as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if held < intent.qty as f64 {
                    return Err(
                        "No hay contratos largos disponibles para cerrar esa cantidad".into(),
                    );
                }
            }
            if let Some(t) = quote_ms {
                let now = crate::live::now_ms();
                if t > now || now.saturating_sub(t) > 5000 {
                    return Err("Cotización caducada antes del envío; no se envió la orden".into());
                }
            }
            Ok::<(), String>(())
        }
        .await;
        preflight.map_err(|message| SubmissionError {
            message,
            attempted: false,
        })?;
        self.request(Method::POST, "/orders", &[], Some(intent.payload()))
            .await
            .map_err(|message| SubmissionError {
                message,
                attempted: true,
            })
    }
    pub async fn cancel(&self, client_id: &str) -> Result<Value, String> {
        let order = self.order(client_id).await?;
        let id = order["id"].as_str().ok_or("Orden no válida")?;
        if !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err("Identificador de orden no válido".into());
        }
        self.request(Method::DELETE, &format!("/orders/{id}"), &[], None)
            .await
    }
}
fn http_error(code: u16) -> String {
    match code {
        401 => "Alpaca rechazó las credenciales Paper",
        403 => "Alpaca denegó el permiso solicitado",
        404 => "Recurso u orden no encontrado en Alpaca Paper",
        422 => "Alpaca rechazó los parámetros de la orden",
        429 => "Límite de consultas de Alpaca; espera antes de reintentar",
        _ => "Error del servicio Alpaca Paper",
    }
    .to_owned()
        + &format!(" (HTTP {code})")
}
fn valid_date(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}
fn validate_id(s: &str) -> Result<(), String> {
    if !s.starts_with("snipers-")
        || s.len() > 48
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "client_order_id debe comenzar por snipers- y tener hasta 48 caracteres".into(),
        );
    }
    Ok(())
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderIntent {
    pub symbol: String,
    pub qty: u32,
    pub limit_price: String,
    pub side: OrderSide,
    pub client_order_id: String,
}
impl OrderIntent {
    pub fn validate(&self) -> Result<(), String> {
        validate_id(&self.client_order_id)?;
        let n = self.symbol.len();
        let valid_symbol = self.symbol.is_ascii() && (16..=21).contains(&n) && {
            let (root, tail) = self.symbol.split_at(n - 15);
            root.bytes().all(|b| b.is_ascii_uppercase())
                && tail[..6].bytes().all(|b| b.is_ascii_digit())
                && matches!(tail.as_bytes()[6], b'C' | b'P')
                && tail[7..].bytes().all(|b| b.is_ascii_digit())
        };
        let price = self.limit_price.parse::<f64>().unwrap_or(f64::NAN);
        if !valid_symbol
            || self.qty == 0
            || !price.is_finite()
            || price <= 0.0
            || !self
                .limit_price
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
        {
            return Err("Orden de opción inválida: contrato OCC, cantidad entera y prima límite positiva requeridos".into());
        }
        Ok(())
    }
    fn payload(&self) -> Value {
        json!({"symbol":self.symbol,"qty":self.qty.to_string(),"side":self.side,"type":"limit","time_in_force":"day","limit_price":self.limit_price,"client_order_id":self.client_order_id,"position_intent":if self.side==OrderSide::Buy {"buy_to_open"} else {"sell_to_close"}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn intent() -> OrderIntent {
        OrderIntent {
            symbol: "QQQ260925C00600000".into(),
            qty: 1,
            limit_price: "1.25".into(),
            side: OrderSide::Buy,
            client_order_id: "snipers-test-001".into(),
        }
    }
    #[test]
    fn reject_live_credentials() {
        assert!(Credentials::new("AK1234567890123456".into(), "a".repeat(40)).is_err());
    }
    #[test]
    fn option_only() {
        for s in [
            "QQQ",
            "../account",
            "QQQ260925X00600000",
            "🦁260925C00600000",
        ] {
            let mut i = intent();
            i.symbol = s.into();
            assert!(i.validate().is_err());
        }
    }
    #[test]
    fn invalid_risk_inputs() {
        for p in ["0", "-1", "NaN", "inf", "1e9"] {
            let mut i = intent();
            i.limit_price = p.into();
            assert!(i.validate().is_err());
        }
        let mut i = intent();
        i.qty = 0;
        assert!(i.validate().is_err());
    }
    #[test]
    fn explicit_long_only_intent() {
        let mut i = intent();
        assert!(i.validate().is_ok());
        assert_eq!(i.payload()["position_intent"], "buy_to_open");
        i.side = OrderSide::Sell;
        assert_eq!(i.payload()["position_intent"], "sell_to_close");
        assert_eq!(i.payload()["type"], "limit");
    }
    #[test]
    fn scoped_order_ids() {
        assert!(validate_id("someone-elses-order").is_err());
        assert!(validate_id("snipers-../x").is_err());
        assert!(validate_id(&"x".repeat(49)).is_err());
    }
    #[test]
    fn errors_do_not_echo_response() {
        assert!(http_error(401).contains("401"));
        assert!(http_error(429).contains("Límite"));
    }
    #[cfg(windows)]
    #[test]
    fn encrypted_roundtrip() {
        let secret = "test-only-never-a-real-key";
        let ciphertext = dpapi(secret, true).unwrap();
        assert!(!ciphertext.contains(secret));
        assert_eq!(dpapi(&ciphertext, false).unwrap(), secret);
        assert!(dpapi("invalid", false).is_err());
    }

    fn mock(responses: Vec<(u16, Value)>) -> (PaperClient, std::thread::JoinHandle<Vec<String>>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = vec![];
            for (code, json) in responses {
                let (mut s, _) = listener.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut raw = vec![];
                let mut buf = [0; 4096];
                loop {
                    let n = s.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let h = String::from_utf8_lossy(&raw[..end]);
                        let len = h
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if raw.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(raw).unwrap());
                let body = if code == 204 {
                    String::new()
                } else {
                    json.to_string()
                };
                write!(s,"HTTP/1.1 {code} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
            requests
        });
        let mut client = PaperClient::new(
            Credentials::new("PK1234567890123456".into(), "x".repeat(40)).unwrap(),
        )
        .unwrap();
        client.test_base = Some(base);
        (client, server)
    }
    fn active() -> Value {
        json!({"status":"ACTIVE","trading_blocked":false,"account_blocked":false,"options_trading_level":3})
    }
    #[tokio::test]
    async fn authenticated_request_and_redacted_errors() {
        for code in [401, 403, 422, 429, 500] {
            let (c, s) = mock(vec![(
                code,
                json!({"message":"secret-response-must-not-leak"}),
            )]);
            let e = c.account().await.unwrap_err();
            assert!(!e.contains("secret-response"));
            assert!(e.contains(&code.to_string()));
            let r = s.join().unwrap();
            assert!(r[0]
                .to_lowercase()
                .contains("apca-api-key-id: pk1234567890123456"));
        }
    }
    #[tokio::test]
    async fn blocked_account_never_submits() {
        let mut a = active();
        a["trading_blocked"] = json!(true);
        let (c, s) = mock(vec![(200, a)]);
        assert!(c.submit(intent()).await.unwrap_err().contains("no permite"));
        assert_eq!(s.join().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn closed_market_never_submits() {
        let (c, s) = mock(vec![(200, active()), (200, json!({"is_open":false}))]);
        assert!(c
            .submit(intent())
            .await
            .unwrap_err()
            .contains("Mercado cerrado"));
        assert_eq!(s.join().unwrap().len(), 2);
    }
    #[tokio::test]
    async fn preflight_failure_is_distinct_from_uncertain_post() {
        let (c, s) = mock(vec![
            (200, active()),
            (200, json!({"is_open":true})),
            (200, json!({"tradable":true,"status":"active"})),
        ]);
        let failure = c.submit_outcome(intent(), Some(1)).await.err().unwrap();
        assert!(!failure.attempted);
        assert!(s.join().unwrap().iter().all(|r| r.starts_with("GET ")));
        let (c, s) = mock(vec![
            (200, active()),
            (200, json!({"is_open":true})),
            (200, json!({"tradable":true,"status":"active"})),
            (500, json!({})),
        ]);
        let failure = c.submit_outcome(intent(), None).await.err().unwrap();
        assert!(failure.attempted);
        assert!(s.join().unwrap()[3].starts_with("POST /orders"));
    }
    #[tokio::test]
    async fn buy_submits_exact_limit_and_reconciles() {
        let result =
            json!({"id":"abc-123","client_order_id":"snipers-test-001","status":"accepted"});
        let (c, s) = mock(vec![
            (200, active()),
            (200, json!({"is_open":true})),
            (200, json!({"tradable":true,"status":"active"})),
            (200, result.clone()),
            (200, result),
        ]);
        assert_eq!(c.submit(intent()).await.unwrap()["status"], "accepted");
        assert_eq!(c.order("snipers-test-001").await.unwrap()["id"], "abc-123");
        let r = s.join().unwrap();
        let payload: Value = serde_json::from_str(r[3].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(payload, intent().payload());
        assert!(r[4].starts_with("GET /orders:by_client_order_id?client_order_id=snipers-test-001"));
    }
    #[tokio::test]
    async fn cannot_sell_without_available_long_position() {
        let (c, s) = mock(vec![
            (200, active()),
            (200, json!({"is_open":true})),
            (200, json!({"tradable":true,"status":"active"})),
            (200, json!([])),
        ]);
        let mut i = intent();
        i.side = OrderSide::Sell;
        assert!(c.submit(i).await.unwrap_err().contains("No hay contratos"));
        assert_eq!(s.join().unwrap().len(), 4);
    }
    #[tokio::test]
    async fn cancel_is_scoped_and_handles_empty_204() {
        let (c, s) = mock(vec![(200, json!({"id":"abc-123"})), (204, Value::Null)]);
        assert_eq!(c.cancel("snipers-test-001").await.unwrap(), Value::Null);
        assert!(s.join().unwrap()[1].starts_with("DELETE /orders/abc-123 "));
    }
    #[tokio::test]
    async fn contracts_preserve_expiration_and_pagination() {
        let (c, s) = mock(vec![(
            200,
            json!({"option_contracts":[],"next_page_token":"next"}),
        )]);
        c.contracts("QQQ", "2026-09-25", Some("a+b="))
            .await
            .unwrap();
        let r = s.join().unwrap();
        assert!(r[0].contains("expiration_date=2026-09-25"));
        assert!(r[0].contains("page_token=a%2Bb%3D"));
    }
}
