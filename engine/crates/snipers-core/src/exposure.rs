//! Estimated exposure, not dealer inventory. Pure inputs and explicit model clock.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub kind: String,
    pub strike: f64,
    pub oi: Option<f64>,
    pub gamma: Option<f64>,
    pub delta: Option<f64>,
    pub iv: Option<f64>,
    /// Verified monetary multiplier. Adjusted/unknown contracts must remain None.
    pub multiplier: Option<f64>,
    pub years: Option<f64>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub risk_free_rate: f64,
    pub dividend_yield: f64,
    pub scan_percent: f64,
    pub scan_steps: usize,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            risk_free_rate: 0.04,
            dividend_yield: 0.005,
            scan_percent: 0.10,
            scan_steps: 241,
        }
    }
}
#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Strike {
    pub strike: f64,
    pub call_gex: f64,
    pub put_gex: f64,
    pub net_gex: f64,
    pub call_dex: f64,
    pub put_dex: f64,
    pub net_dex: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub spot: f64,
    pub net_gex: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exposure {
    pub model: Model,
    pub spot: f64,
    pub contracts: usize,
    pub eligible: usize,
    pub unknown_oi: usize,
    pub unsupported: usize,
    pub gamma_used: usize,
    pub delta_used: usize,
    pub scan_used: usize,
    pub gex_complete: bool,
    pub dex_complete: bool,
    pub scan_complete: bool,
    pub call_gex: f64,
    pub put_gex: f64,
    pub net_gex: f64,
    pub gamma_regime: String,
    pub call_dex: f64,
    pub put_dex: f64,
    pub net_dex: f64,
    pub dex_side: String,
    pub dex_bias: Option<f64>,
    pub call_dex_pct: Option<f64>,
    pub put_dex_pct: Option<f64>,
    pub delta_coverage_pct: Option<f64>,
    /// Bounds within the supplied chain only; absent for unknown OI/multipliers.
    pub call_share_bounds: Option<[f64; 2]>,
    pub missing_call_dex_max: Option<f64>,
    pub missing_put_dex_max: Option<f64>,
    pub delta_capacity_coverage_pct: Option<f64>,
    pub zero_gamma: Option<f64>,
    pub crossings: Vec<f64>,
    pub grid: Vec<Point>,
    pub strikes: Vec<Strike>,
}
fn positive(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > 0.0)
}
fn nonnegative(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v >= 0.0)
}
fn sign(c: &Contract) -> Option<f64> {
    match c.kind.as_str() {
        "call" => Some(1.0),
        "put" => Some(-1.0),
        _ => None,
    }
}

impl Model {
    pub fn gamma(&self, spot: f64, strike: f64, iv: f64, years: f64) -> Option<f64> {
        if [spot, strike, iv, years]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || !self.risk_free_rate.is_finite()
            || !self.dividend_yield.is_finite()
        {
            return None;
        }
        let denominator = iv * years.sqrt();
        let d1 = ((spot / strike).ln()
            + (self.risk_free_rate - self.dividend_yield + 0.5 * iv * iv) * years)
            / denominator;
        let value = (-self.dividend_yield * years).exp() * (-0.5 * d1 * d1).exp()
            / (std::f64::consts::TAU.sqrt() * spot * denominator);
        value.is_finite().then_some(value)
    }

    pub fn calculate(&self, contracts: &[Contract], spot: f64) -> Result<Exposure, &'static str> {
        if positive(Some(spot)).is_none()
            || !(51..=1001).contains(&self.scan_steps)
            || !self.scan_percent.is_finite()
            || !(0.0..1.0).contains(&self.scan_percent)
            || self.scan_percent == 0.0
            || !self.risk_free_rate.is_finite()
            || !self.dividend_yield.is_finite()
            || contracts.len() > 20_000
        {
            return Err("Parámetros del modelo de exposición inválidos");
        }
        let mut output = Exposure {
            model: *self,
            spot,
            contracts: contracts.len(),
            eligible: 0,
            unknown_oi: 0,
            unsupported: 0,
            gamma_used: 0,
            delta_used: 0,
            scan_used: 0,
            gex_complete: false,
            dex_complete: false,
            scan_complete: false,
            call_gex: 0.0,
            put_gex: 0.0,
            net_gex: 0.0,
            gamma_regime: "NO DATA".into(),
            call_dex: 0.0,
            put_dex: 0.0,
            net_dex: 0.0,
            dex_side: "NO DATA".into(),
            dex_bias: None,
            call_dex_pct: None,
            put_dex_pct: None,
            delta_coverage_pct: None,
            call_share_bounds: None,
            missing_call_dex_max: None,
            missing_put_dex_max: None,
            delta_capacity_coverage_pct: None,
            zero_gamma: None,
            crossings: vec![],
            grid: vec![],
            strikes: vec![],
        };
        let mut ordered: Vec<_> = contracts
            .iter()
            .filter(|c| sign(c).is_some() && positive(Some(c.strike)).is_some())
            .collect();
        ordered.sort_by(|a, b| a.strike.total_cmp(&b.strike));
        output.unsupported = contracts.len() - ordered.len();
        let mut bounded = output.unsupported == 0;
        let (mut missing_call, mut missing_put, mut capacity, mut covered_capacity) = (0.0, 0.0, 0.0, 0.0);
        for c in &ordered {
            let Some(oi) = nonnegative(c.oi) else { bounded = false; continue; };
            if oi == 0.0 { continue; }
            let Some(multiplier) = positive(c.multiplier) else { bounded = false; continue; };
            let cap = oi * multiplier;
            if !cap.is_finite() { return Err("Capacidad delta fuera de rango"); }
            capacity += cap;
            if c.delta.is_some_and(|v| v.is_finite() && v.abs() <= 1.0) { covered_capacity += cap; }
            else if sign(c) == Some(1.0) { missing_call += cap; }
            else { missing_put += cap; }
        }
        if ![capacity, covered_capacity, missing_call, missing_put].iter().all(|v| v.is_finite()) {
            return Err("Capacidad delta acumulada fuera de rango");
        }
        let mut scan = Vec::new();
        for c in ordered {
            let Some(oi) = nonnegative(c.oi) else {
                output.unknown_oi += 1;
                continue;
            };
            if oi == 0.0 {
                continue;
            }
            output.eligible += 1;
            if output.strikes.last().is_none_or(|s| s.strike != c.strike) {
                output.strikes.push(Strike {
                    strike: c.strike,
                    ..Default::default()
                });
            }
            let row = output.strikes.last_mut().unwrap();
            let Some(multiplier) = positive(c.multiplier) else {
                continue;
            };
            let direction = sign(c).unwrap();
            if let Some(gamma) = nonnegative(c.gamma) {
                let gex = gamma * oi * multiplier * spot * spot * 0.01;
                if !gex.is_finite() {
                    return Err("Exposición gamma fuera de rango");
                }
                output.gamma_used += 1;
                if direction > 0.0 {
                    row.call_gex += gex;
                    output.call_gex += gex;
                } else {
                    row.put_gex += gex;
                    output.put_gex += gex;
                }
            }
            if let Some(delta) = c.delta.filter(|v| v.is_finite() && v.abs() <= 1.0) {
                let dex = direction * delta.abs() * oi * multiplier;
                if !dex.is_finite() {
                    return Err("Exposición delta fuera de rango");
                }
                output.delta_used += 1;
                if direction > 0.0 {
                    row.call_dex += dex;
                    output.call_dex += dex;
                } else {
                    row.put_dex += dex;
                    output.put_dex += dex;
                }
            }
            if let (Some(iv), Some(years)) = (positive(c.iv), positive(c.years)) {
                scan.push((c.strike, iv, years, direction * oi * multiplier));
            }
        }
        output.scan_used = scan.len();
        output.gex_complete = output.eligible > 0
            && output.unknown_oi == 0
            && output.unsupported == 0
            && output.gamma_used == output.eligible;
        output.dex_complete = output.eligible > 0
            && output.unknown_oi == 0
            && output.unsupported == 0
            && output.delta_used == output.eligible;
        output.scan_complete = output.eligible > 0
            && output.unknown_oi == 0
            && output.unsupported == 0
            && output.scan_used == output.eligible;
        output.net_gex = output.call_gex - output.put_gex;
        output.net_dex = output.call_dex + output.put_dex;
        let gross = output.call_dex.abs() + output.put_dex.abs();
        if ![
            output.call_gex,
            output.put_gex,
            output.net_gex,
            output.call_dex,
            output.put_dex,
            gross,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err("Suma de exposiciones fuera de rango");
        }
        output.gamma_regime = if !output.gex_complete {
            "INCOMPLETE"
        } else if output.net_gex > 0.0 {
            "POSITIVE GAMMA"
        } else if output.net_gex < 0.0 {
            "NEGATIVE GAMMA"
        } else {
            "NEUTRAL GAMMA"
        }
        .into();
        if output.eligible > 0 {
            output.delta_coverage_pct =
                Some(output.delta_used as f64 / output.eligible as f64 * 100.0);
        }
        if bounded && capacity > 0.0 {
            output.missing_call_dex_max = Some(missing_call);
            output.missing_put_dex_max = Some(missing_put);
            output.delta_capacity_coverage_pct = Some(100.0 * covered_capacity / capacity);
            if gross > 0.0 {
                let low_den = gross + missing_put;
                let high_den = gross + missing_call;
                if low_den.is_finite() && high_den.is_finite() {
                    output.call_share_bounds = Some([
                        100.0 * (output.call_dex / low_den),
                        100.0 * ((output.call_dex + missing_call) / high_den),
                    ]);
                }
            }
        }
        if gross > 0.0 && output.dex_complete {
            let bias = output.net_dex / gross;
            output.dex_bias = Some(bias);
            output.call_dex_pct = Some(output.call_dex.abs() / gross * 100.0);
            output.put_dex_pct = Some(output.put_dex.abs() / gross * 100.0);
            output.dex_side = if bias >= 0.05 {
                "CALL"
            } else if bias <= -0.05 {
                "PUT"
            } else {
                "NEUTRAL"
            }
            .into();
        } else if !output.dex_complete {
            output.dex_side = "INCOMPLETE".into();
        }
        for row in &mut output.strikes {
            row.net_gex = row.call_gex - row.put_gex;
            row.net_dex = row.call_dex + row.put_dex;
        }
        // A partial chain, a flat zero curve, or underflow alone cannot certify a crossing.
        if output.scan_complete {
            let low = spot * (1.0 - self.scan_percent);
            let high = spot * (1.0 + self.scan_percent);
            let step = (high - low) / (self.scan_steps - 1) as f64;
            for i in 0..self.scan_steps {
                let price = low + step * i as f64;
                let mut net = 0.0;
                for &(strike, iv, years, weight) in &scan {
                    net += self
                        .gamma(price, strike, iv, years)
                        .ok_or("Gamma teórica fuera de rango")?
                        * weight
                        * price
                        * price
                        * 0.01;
                }
                if !net.is_finite() {
                    return Err("Curva gamma fuera de rango");
                }
                output.grid.push(Point {
                    spot: price,
                    net_gex: net,
                });
            }
            for pair in output.grid.windows(2) {
                let (left, right) = (&pair[0], &pair[1]);
                if left.net_gex != 0.0
                    && right.net_gex != 0.0
                    && left.net_gex.is_sign_positive() != right.net_gex.is_sign_positive()
                {
                    let (a, b) = (left.net_gex.abs(), right.net_gex.abs());
                    let ratio = if a >= b {
                        1.0 / (1.0 + b / a)
                    } else {
                        (a / b) / (1.0 + a / b)
                    };
                    output
                        .crossings
                        .push(left.spot + ratio * (right.spot - left.spot));
                }
            }
            for triple in output.grid.windows(3) {
                if triple[1].net_gex == 0.0
                    && triple[0].net_gex != 0.0
                    && triple[2].net_gex != 0.0
                    && triple[0].net_gex.is_sign_positive() != triple[2].net_gex.is_sign_positive()
                {
                    output.crossings.push(triple[1].spot);
                }
            }
            output.crossings.sort_by(f64::total_cmp);
            output.zero_gamma = output
                .crossings
                .iter()
                .copied()
                .min_by(|a, b| (a - spot).abs().total_cmp(&(b - spot).abs()));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_delta_bounds_weight_oi_not_series_count() {
        let row = |kind: &str, oi, delta| Contract {kind:kind.into(),strike:100.0,oi:Some(oi),delta,gamma:None,iv:None,multiplier:Some(100.0),years:None};
        let mut rows = vec![row("call",10.0,Some(0.8)),row("put",10.0,Some(-0.2)),row("put",100.0,None)];
        let e=Model::default().calculate(&rows,100.0).unwrap();
        let b=e.call_share_bounds.unwrap();
        assert!((b[0]-100.0*800.0/11000.0).abs()<1e-8);
        assert!((b[1]-80.0).abs()<1e-8);
        assert!(e.delta_capacity_coverage_pct.unwrap()<17.0);
        rows[2].oi=Some(0.1);
        assert!(Model::default().calculate(&rows,100.0).unwrap().call_share_bounds.unwrap()[0]>52.5);
        rows[2].oi=None;
        assert!(Model::default().calculate(&rows,100.0).unwrap().call_share_bounds.is_none());
        rows[2].oi=Some(1.0);rows[2].multiplier=None;
        assert!(Model::default().calculate(&rows,100.0).unwrap().call_share_bounds.is_none());
    }
    use serde_json::Value;
    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    }
    #[test]
    fn python_exposures_and_non_degenerate_crossings_match() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/legacy-exposure.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let contracts: Vec<_> = case["contracts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| Contract {
                    kind: c["type"].as_str().unwrap().into(),
                    strike: c["strike"].as_f64().unwrap(),
                    oi: c["open_interest"].as_f64(),
                    gamma: c["gamma"].as_f64(),
                    delta: c["delta"].as_f64(),
                    iv: c["implied_volatility"].as_f64(),
                    multiplier: Some(100.0),
                    years: fixture["expiryYears"][c["expiration"].as_str().unwrap()].as_f64(),
                })
                .collect();
            let actual = Model::default().calculate(&contracts, 500.0).unwrap();
            for (value, key) in [
                (actual.call_gex, "call_gex"),
                (actual.put_gex, "put_gex"),
                (actual.net_gex, "net_gex"),
            ] {
                if case["gex_error"].is_string() {
                    assert!(!actual.gex_complete);
                    assert_eq!(actual.gamma_regime, "INCOMPLETE");
                } else {
                    close(value, case["gex"][key].as_f64().unwrap());
                }
            }
            for (value, key) in [
                (actual.call_dex, "call_dex"),
                (actual.put_dex, "put_dex"),
                (actual.net_dex, "net_dex"),
            ] {
                close(value, case["dex"][key].as_f64().unwrap());
            }
            if let Some(gamma) = actual.zero_gamma {
                close(gamma, case["gex"]["zero_gamma"].as_f64().unwrap());
            }
            if actual.scan_complete {
                let expected = case["scan"]["grid"].as_array().unwrap();
                assert_eq!(actual.grid.len(), expected.len());
                for (point, reference) in actual.grid.iter().zip(expected) {
                    close(point.spot, reference["spot"].as_f64().unwrap());
                    close(point.net_gex, reference["net_gex"].as_f64().unwrap());
                }
            }
            for strike in &actual.strikes {
                if let Some(rows) = case["gex"]["strikes"].as_array() {
                    let expected = rows
                        .iter()
                        .find(|r| r["strike"].as_f64() == Some(strike.strike))
                        .unwrap();
                    close(strike.net_gex, expected["net_gex"].as_f64().unwrap());
                } else {
                    assert!(case["gex_error"].is_string());
                    assert!(!actual.gex_complete);
                }
                if let Some(expected) = case["dex"]["strikes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["strike"].as_f64() == Some(strike.strike))
                {
                    close(strike.net_dex, expected["net_dex"].as_f64().unwrap());
                }
            }
            if case["name"] == "crossing" || case["name"] == "mixed_expirations" {
                assert!(actual.zero_gamma.is_some());
            }
            if actual.dex_complete && actual.dex_bias.is_some() {
                assert_eq!(actual.dex_side, case["dex"]["dex_side"].as_str().unwrap());
            }
            if ["empty", "zero_oi", "missing_greeks", "balanced_identical"]
                .iter()
                .any(|n| case["name"] == *n)
            {
                assert_eq!(actual.zero_gamma, None);
            }
        }
    }
    #[test]
    fn missing_inputs_cannot_authorize_a_regime_and_numeric_overflow_is_rejected() {
        let mut c = Contract {
            kind: "call".into(),
            strike: 500.0,
            oi: Some(100.0),
            gamma: Some(0.02),
            delta: Some(0.5),
            iv: Some(0.25),
            multiplier: None,
            years: Some(1.0 / 365.0),
        };
        let result = Model::default().calculate(&[c.clone()], 500.0).unwrap();
        assert!(!result.gex_complete && !result.dex_complete && !result.scan_complete);
        assert_eq!(result.zero_gamma, None);
        assert_eq!(result.dex_bias, None);
        c.multiplier = Some(100.0);
        c.oi = None;
        assert_eq!(
            Model::default()
                .calculate(&[c.clone()], 500.0)
                .unwrap()
                .unknown_oi,
            1
        );
        c.oi = Some(f64::MAX);
        assert!(Model::default().calculate(&[c], 500.0).is_err());
        assert!(Model::default().calculate(&[], f64::NAN).is_err());
    }
}
