//! Session supervisor and NQ publication boundary. Desktop stays unlicensed.
use crate::{
    cme::{Connector, Error, Product},
    cme_cache::Cache,
    cme_data::{Instrument, Observation},
};
use serde::Serialize;
use snipers_core::synthetic::{self, Conversion, ReferencePrice};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: String,
    pub detail: String,
    pub environment: String,
    pub contract: Option<String>,
    pub nq_price: Option<f64>,
    pub nq_timestamp_ms: Option<u64>,
    pub conversions: Vec<Conversion>,
    pub reconnects: u64,
    pub entry_confirmed: bool,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            phase: "pending_license".into(),
            detail: "CME pendiente de licencia".into(),
            environment: "INACTIVE".into(),
            contract: None,
            nq_price: None,
            nq_timestamp_ms: None,
            conversions: Vec::new(),
            reconnects: 0,
            entry_confirmed: false,
        }
    }
}
pub struct Session {
    cache: Cache,
    instrument: Instrument,
    environment: String,
    connected: bool,
    nq: Option<ReferencePrice>,
    reconnects: u64,
}
impl Session {
    /// The full contract identity must come from the selected CME reference.
    /// No nearest-contract inference, spread substitution, or auto roll.
    pub fn new(licensed: bool, environment: &str, instrument: Instrument) -> Result<Self, Error> {
        if !licensed {
            return Err(Error::PendingLicense);
        }
        crate::cme::Environment::parse(environment)?;
        if instrument.exchange != "XCME"
            || instrument.product_code != "NQ"
            || instrument.product_type != "FUT"
            || instrument.id.is_empty()
            || instrument.segment.is_empty()
            || !outright(&instrument.symbol)
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            cache: Cache::default(),
            instrument,
            environment: environment.into(),
            connected: false,
            nq: None,
            reconnects: 0,
        })
    }
    pub fn begin(&mut self) {
        self.cache.begin_session();
        self.nq = None;
        self.connected = false;
    }
    pub fn disconnect(&mut self) {
        if self.connected {
            self.reconnects = self.reconnects.saturating_add(1);
        }
        self.cache.invalidate();
        self.nq = None;
        self.connected = false;
    }
    /// Only called for the transport's authenticated, acknowledged market stream.
    pub fn receive(&mut self, message: &serde_json::Value, now: u64) -> Result<(), Error> {
        let result = self.receive_inner(message, now);
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn receive_inner(&mut self, message: &serde_json::Value, now: u64) -> Result<(), Error> {
        self.cache.apply(message)?;
        self.connected = true;
        let Some(Observation::Trade {
            instrument,
            price,
            last_update,
            ..
        }) = self
            .cache
            .get(
                &self.instrument.exchange,
                &self.instrument.segment,
                &self.instrument.id,
            )
            .and_then(|s| s.trade.as_ref())
        else {
            return Ok(());
        };
        if instrument != &self.instrument {
            return Err(Error::Protocol);
        }
        let timestamp_ms =
            snipers_core::calendar::rfc3339_ms(last_update).ok_or(Error::Protocol)?;
        if timestamp_ms > now || !price.is_finite() || *price <= 0.0 {
            return Err(Error::Protocol);
        }
        if self
            .nq
            .as_ref()
            .is_some_and(|old| timestamp_ms < old.timestamp_ms)
        {
            return Ok(());
        }
        self.nq = Some(ReferencePrice {
            symbol: "NQ".into(),
            price: *price,
            timestamp_ms,
            verified: true,
        });
        Ok(())
    }
    pub fn snapshot(&self, qqq: Option<&ReferencePrice>, levels: &[f64], now: u64) -> Status {
        let mut out = Status {
            phase: "waiting".into(),
            detail: "Esperando un trade NQ vigente del contrato seleccionado".into(),
            environment: self.environment.clone(),
            contract: Some(self.instrument.symbol.clone()),
            reconnects: self.reconnects,
            ..Status::default()
        };
        if !self.connected {
            out.phase = "disconnected".into();
            out.detail = "Sin sesión CME vigente".into();
            return out;
        }
        let Some(nq) = self.nq.as_ref().filter(|p| {
            p.timestamp_ms <= now
                && now - p.timestamp_ms <= 15_000
                && snipers_core::calendar::date_at(p.timestamp_ms)
                    == snipers_core::calendar::date_at(now)
        }) else {
            return out;
        };
        out.nq_price = Some(nq.price);
        out.nq_timestamp_ms = Some(nq.timestamp_ms);
        out.phase = "nq_ready".into();
        out.detail = "NQ vigente; conversión pendiente de QQQ y niveles estructurales".into();
        if let Some(qqq) = qqq.filter(|_| !levels.is_empty() && levels.len() <= 128) {
            if let Ok(rows) = levels
                .iter()
                .map(|level| synthetic::nq_to_qqq(*level, nq, qqq, now))
                .collect::<Result<Vec<_>, _>>()
            {
                out.conversions = rows;
                out.phase = "ready".into();
                out.detail = "Conversión aproximada NQ→QQQ con precios sincronizados".into();
            }
        }
        out
    }
}
fn outright(symbol: &str) -> bool {
    let Some(suffix) = symbol.strip_prefix("NQ") else {
        return false;
    };
    let bytes = suffix.as_bytes();
    (2..=5).contains(&bytes.len())
        && b"HMUZ".contains(&bytes[0])
        && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// Drops the socket future on malformed data or cancellation; every retry starts
/// a fresh cache and obtains a new token. No credential or raw body is published.
pub async fn supervise(
    connector: &Connector,
    products: &[Product],
    session: &mut Session,
    mut publish: impl FnMut(&Session),
) {
    if connector.environment_name() != session.environment
        || !products.iter().any(|p| p.code == "NQ" && !p.options)
    {
        session.disconnect();
        publish(session);
        return;
    }
    loop {
        let (tx, rx) = tokio::sync::mpsc::channel(512);
        consume_session(session, connector.run(products, tx), rx, &mut publish).await;
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
async fn consume_session(
    session: &mut Session,
    transport: impl std::future::Future<Output = Result<(), Error>>,
    mut rx: tokio::sync::mpsc::Receiver<serde_json::Value>,
    publish: &mut impl FnMut(&Session),
) {
    session.begin();
    publish(session);
    tokio::pin!(transport);
    loop {
        tokio::select! {
            biased;
            _=&mut transport=>break,
            message=rx.recv()=>{
                let Some(message)=message else {break;};
                if session.receive(&message,crate::live::now_ms()).is_err(){break;}
                publish(session);
            }
        }
    }
    session.disconnect();
    publish(session);
}

/// Called by the session supervisor after a market update or disconnect. Reads
/// only QQQ's own verified trade lane; the visible asset is never substituted.
pub fn publish_to_live(state: &crate::live::SharedState, session: &Session, levels: &[f64]) {
    let now = crate::live::now_ms();
    let mut s = state.lock().unwrap();
    let qqq = s.ticks.get("QQQ").and_then(|t| {
        Some(ReferencePrice {
            symbol: t.symbol.clone(),
            price: t.price?,
            timestamp_ms: t.trade_ms?,
            verified: s.stocks.phase == "streaming",
        })
    });
    s.cme = session.snapshot(qqq.as_ref(), levels, now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn supervisor_publishes_disconnect_and_requires_new_session_data() {
        let mut s = Session::new(true, "UAT", instrument()).unwrap();
        for _ in 0..2 {
            let (tx, rx) = tokio::sync::mpsc::channel(4);
            tx.send(message(1, "2000-01-01T00:00:00Z", 20000.0))
                .await
                .unwrap();
            // A sequence gap must terminate the consumer, even while transport lives.
            tx.send(message(3, "2000-01-01T00:00:00Z", 20001.0))
                .await
                .unwrap();
            let mut states = Vec::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                consume_session(&mut s, std::future::pending(), rx, &mut |s| {
                    states.push((s.connected, s.nq.is_some()))
                }),
            )
            .await
            .unwrap();
            assert_eq!(states, vec![(false, false), (true, true), (false, false)]);
            drop(tx);
        }
        assert_eq!(s.reconnects, 2);
    }
    fn now() -> u64 {
        snipers_core::calendar::rfc3339_ms("2026-09-24T15:00:00Z").unwrap()
    }
    fn instrument() -> Instrument {
        Instrument {
            id: "123".into(),
            exchange: "XCME".into(),
            segment: "54".into(),
            symbol: "NQZ6".into(),
            product_code: "NQ".into(),
            product_type: "FUT".into(),
        }
    }
    fn message(seq: u64, time: &str, price: f64) -> serde_json::Value {
        json!({"header":{"messageType":"TRD","sequenceNumber":seq},"payload":[{"instrument":{"id":"123","exchangeMic":"XCME","marketSegmentId":"54","symbol":"NQZ6","productCode":"NQ","productType":"FUT"},"lastUpdateTime":time,"tradeSummary":{"tradePrice":price,"tradeQty":1}}]})
    }
    fn qqq() -> ReferencePrice {
        ReferencePrice {
            symbol: "QQQ".into(),
            price: 500.0,
            timestamp_ms: now(),
            verified: true,
        }
    }
    #[test]
    fn license_contract_identity_and_vendor_clock_are_explicit() {
        assert!(matches!(
            Session::new(false, "BAD", instrument()),
            Err(Error::PendingLicense)
        ));
        for symbol in ["NQZ6-NQH7", "NQ", "MNQZ6", "NQZ6 C20000", "NQF6"] {
            let mut i = instrument();
            i.symbol = symbol.into();
            assert!(Session::new(true, "UAT", i).is_err());
        }
        assert_eq!(
            snipers_core::calendar::rfc3339_ms("2026-09-24T11:00:00.000123456-04:00"),
            Some(now())
        );
        assert!(snipers_core::calendar::rfc3339_ms("2026-09-24 15:00:00").is_none());
        assert!(Status::default().conversions.is_empty());
        assert_eq!(Status::default().phase, "pending_license");
    }
    #[test]
    fn nq_conversion_requires_live_matched_prices_and_never_uses_settlement() {
        let mut s = Session::new(true, "UAT", instrument()).unwrap();
        s.begin();
        assert!(s
            .snapshot(Some(&qqq()), &[20100.0], now())
            .conversions
            .is_empty());
        s.receive(&message(1, "2026-09-24T15:00:00Z", 20000.0), now())
            .unwrap();
        let read = s.snapshot(Some(&qqq()), &[20100.0], now());
        assert_eq!(read.conversions[0].qqq_level, 502.5);
        assert!(!read.entry_confirmed);
        assert!(s
            .snapshot(Some(&qqq()), &[20100.0, f64::NAN], now())
            .conversions
            .is_empty());
        assert!(s
            .snapshot(Some(&qqq()), &[20100.0], now() + 15001)
            .nq_price
            .is_none());
        let mut stale = qqq();
        stale.timestamp_ms -= 2001;
        assert!(s
            .snapshot(Some(&stale), &[20100.0], now())
            .conversions
            .is_empty());
        s.disconnect();
        assert!(s
            .snapshot(Some(&qqq()), &[20100.0], now())
            .nq_price
            .is_none());
        s.begin();
        let stat = json!({"header":{"messageType":"STAT","sequenceNumber":1},"payload":[{"instrument":{"id":"123","exchangeMic":"XCME","marketSegmentId":"54","symbol":"NQZ6","productCode":"NQ","productType":"FUT"},"tradeStatistics":{"settlementPrice":"20000"}}]});
        s.receive(&stat, now()).unwrap();
        assert!(s
            .snapshot(Some(&qqq()), &[20100.0], now())
            .nq_price
            .is_none());
    }
    #[test]
    fn replay_foreign_identity_future_and_sequence_gaps_revoke_or_preserve_correctly() {
        let mut s = Session::new(true, "UAT", instrument()).unwrap();
        s.begin();
        s.receive(&message(1, "2026-09-24T15:00:00Z", 20000.0), now())
            .unwrap();
        s.receive(&message(2, "2026-09-24T14:59:59Z", 19900.0), now())
            .unwrap();
        assert_eq!(s.snapshot(None, &[], now()).nq_price, Some(20000.0));
        assert!(s
            .receive(&message(4, "2026-09-24T15:00:00Z", 20000.0), now())
            .is_err());
        assert!(s.snapshot(None, &[], now()).nq_price.is_none());
        for mode in 0..4 {
            s.begin();
            let mut m = message(1, "2026-09-24T15:00:00Z", 20000.0);
            match mode {
                0 => m["payload"][0]["lastUpdateTime"] = json!("2026-09-24T15:00:01Z"),
                1 => m["payload"][0]["instrument"]["symbol"] = json!("NQH7"),
                2 => m["payload"][0]["lastUpdateTime"] = json!("bad"),
                _ => m["payload"][0]["tradeSummary"]["tradePrice"] = json!(-1),
            };
            assert!(s.receive(&m, now()).is_err());
            assert!(s.snapshot(None, &[], now()).nq_price.is_none());
        }
    }
}
