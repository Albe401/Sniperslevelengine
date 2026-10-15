//! Explainable long-term research screen. No broker actions or price targets.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub id: String,
    pub label: String,
    pub group: String,
    pub value: Option<f64>,
    pub unit: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub label: String,
    pub passed: Option<bool>,
    pub weight: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    #[serde(default)]
    pub opportunity: Option<crate::opportunity::Opportunity>,
    pub ticker: String,
    pub name: String,
    pub industry: String,
    pub sic: String,
    pub cik: String,
    pub as_of: String,
    pub period: String,
    pub filing: String,
    pub price_date: String,
    pub score: Option<f64>,
    pub coverage: f64,
    pub status: String,
    pub metrics: Vec<Metric>,
    pub checks: Vec<Check>,
    pub warnings: Vec<String>,
    pub annual: Vec<Value>,
    pub closes: Vec<f64>,
    pub source_errors: Vec<String>,
    pub eligible: bool,
    #[serde(default)]
    pub research: Option<crate::investment_structure::Research>,
    #[serde(default)]
    pub zones: crate::investment_zones::Zones,
}
pub fn number(v: &Value, k: &str) -> Option<f64> {
    v[k].as_f64().filter(|x| x.is_finite())
}
/// Research policy v2. Thresholds are explicit heuristics, not sector fair values.
/// Called after verified beta and forward estimates have been attached.
pub fn apply_research_metrics(c: &mut Company) {
    const ROE: &str = "ROE ≥ 15%, patrimonio positivo y deuda neta/EBITDA ≤ 3x";
    if c.checks.iter().any(|ch| ch.label == ROE) {
        return;
    }
    let get = |id: &str| {
        c.metrics
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.value)
            .filter(|v| v.is_finite())
    };
    let roe = get("return_on_equity")
        .zip(get("equity"))
        .zip(get("net_debt_ebitda"))
        .map(|((roe, equity), debt)| roe >= 15.0 && equity > 0.0 && debt <= 3.0);
    let beta = get("beta").map(|v| v.abs() <= 1.5);
    let forward = get("forward_pe").map(|v| v > 0.0 && v <= 25.0);
    for ch in &mut c.checks {
        ch.weight *= 0.8;
    }
    for (label, passed, weight) in [
        (ROE, roe, 10.0),
        ("Sensibilidad beta absoluta ≤ 1,5 vs SPY", beta, 5.0),
        ("P/E forward positivo ≤ 25x", forward, 5.0),
    ] {
        c.checks.push(Check {
            label: label.into(),
            passed,
            weight,
        });
    }
    let score = c
        .checks
        .iter()
        .filter(|c| c.passed == Some(true))
        .map(|c| c.weight)
        .sum::<f64>();
    c.coverage = c
        .checks
        .iter()
        .filter(|c| c.passed.is_some())
        .map(|c| c.weight)
        .sum();
    c.score = c.score.map(|_| score);
    if [roe, beta, forward].iter().any(Option::is_none) {
        c.warnings.push("Modelo v2: faltan ROE verificable, beta o consenso forward; sin puntos en el criterio ausente y menor cobertura, no resultado negativo.".into());
    }
    // Supplemental metrics cannot override invalid identity, stale data, valuation or critical risk gates.
    if matches!(c.status.as_str(), "Candidata a estudiar" | "Vigilar") {
        c.status = if c.coverage < 85.0 {
            "Datos incompletos"
        } else if score >= 75.0 {
            "Candidata a estudiar"
        } else {
            "Vigilar"
        }
        .into();
    }
}
fn ratio(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    let (a, b) = (a?, b?);
    (b > 0.).then_some(a / b).filter(|x| x.is_finite())
}
fn text(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_owned()
}
fn age(s: &str, now: u64) -> Option<u64> {
    let ms = crate::calendar::rfc3339_ms(&format!("{s}T00:00:00Z"))?;
    (ms <= now).then(|| (now - ms) / 86_400_000)
}
fn values(v: &Value) -> Vec<Value> {
    v["results"].as_array().cloned().unwrap_or_default()
}
fn statements(v: &Value, symbol: &str, frame: &str, now: u64) -> Vec<Value> {
    let mut rows = values(v)
        .into_iter()
        .filter(|r| {
            r["timeframe"] == frame
                && r["tickers"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|t| t == symbol))
                && age(&text(r, "period_end"), now).is_some()
                && age(&text(r, "filing_date"), now).is_some()
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|r| std::cmp::Reverse(text(r, "period_end")));
    rows.dedup_by_key(|r| text(r, "period_end"));
    rows
}
fn growth(rows: &[Value], field: &str, years: usize) -> Option<f64> {
    let a = rows.first()?;
    let b = rows.get(years)?;
    let ya = a["fiscal_year"].as_i64()?;
    let yb = b["fiscal_year"].as_i64()?;
    if ya - yb != years as i64 {
        return None;
    }
    let x = number(a, field)?;
    let old = number(b, field)?;
    if old <= 0. || x <= 0. {
        return None;
    }
    Some(((x / old).powf(1. / years as f64) - 1.) * 100.)
}
fn fcf(v: &Value) -> Option<f64> {
    Some(
        number(v, "net_cash_from_operating_activities")?
            - number(v, "purchase_of_property_plant_and_equipment")?.abs(),
    )
}
pub struct Input<'a> {
    pub ratio: &'a Value,
    pub profile: &'a Value,
    pub bars: &'a Value,
    pub income: &'a Value,
    pub cash: &'a Value,
    pub balance: &'a Value,
    pub now: u64,
    pub min_drop: f64,
}
pub fn analyze(input: Input<'_>) -> Company {
    let Input {
        ratio: r,
        profile,
        bars,
        income,
        cash,
        balance,
        now,
        min_drop,
    } = input;
    let symbol = text(r, "ticker");
    let null = Value::Null;
    let p = &profile["results"];
    let ins = statements(income, &symbol, "trailing_twelve_months", now);
    let cfs = statements(cash, &symbol, "trailing_twelve_months", now);
    let bs = statements(balance, &symbol, "quarterly", now);
    let years = statements(income, &symbol, "annual", now);
    let cashyears = statements(cash, &symbol, "annual", now);
    let i = ins.first().unwrap_or(&null);
    let c = cfs.first().unwrap_or(&null);
    let b = bs.first().unwrap_or(&null);
    let aligned =
        text(i, "period_end") == text(c, "period_end") && !text(i, "period_end").is_empty();
    let balance_aligned =
        text(i, "period_end") == text(b, "period_end") && !text(i, "period_end").is_empty();
    let mut warnings = Vec::new();
    let mut m = Vec::new();
    let mut add =
        |id: &str, label: &str, group: &str, value: Option<f64>, unit: &str, source: &str| {
            m.push(Metric {
                id: id.into(),
                label: label.into(),
                group: group.into(),
                value: value.filter(|x| x.is_finite()),
                unit: unit.into(),
                source: source.into(),
            })
        };
    for (key, label, unit) in [
        ("price", "Precio de cierre usado", "usd"),
        ("market_cap", "Capitalización", "usd"),
        ("enterprise_value", "Valor empresa EV", "usd"),
        ("average_volume", "Volumen medio 30 sesiones", "number"),
        ("price_to_earnings", "PER TTM", "x"),
        ("price_to_book", "Precio / valor contable", "x"),
        ("price_to_sales", "Precio / ventas", "x"),
        ("price_to_cash_flow", "Precio / flujo operativo", "x"),
        ("price_to_free_cash_flow", "Precio / flujo libre", "x"),
        ("ev_to_ebitda", "EV / EBITDA", "x"),
        ("ev_to_sales", "EV / ventas", "x"),
        ("earnings_per_share", "BPA del proveedor", "usd"),
        ("dividend_yield", "Rentabilidad por dividendo", "pct"),
        ("return_on_assets", "ROA", "pct"),
        ("return_on_equity", "ROE", "pct"),
        ("debt_to_equity", "Deuda / patrimonio", "x"),
        ("current", "Liquidez corriente", "x"),
        ("quick", "Prueba ácida", "x"),
        ("cash", "Ratio de caja", "x"),
        ("free_cash_flow", "Flujo libre TTM del proveedor", "usd"),
    ] {
        let group = match key {
            "return_on_assets" | "return_on_equity" => "Calidad",
            "debt_to_equity" | "current" | "quick" | "cash" => "Solvencia",
            "free_cash_flow" => "Caja",
            _ => "Valoración",
        };
        add(
            key,
            label,
            group,
            number(r, key).map(|v| if unit == "pct" { v * 100. } else { v }),
            unit,
            "Massive Ratios · cierre/TTM",
        );
    }
    add("shares_outstanding", "Acciones de la clase en circulación", "Valoración", number(p,"share_class_shares_outstanding").filter(|v|*v>0.), "number", "Massive referencia · acciones actuales de esta clase; revisar splits y dilución");
    add(
        "fcf_yield",
        "Rendimiento de flujo libre",
        "Valoración",
        ratio(number(r, "free_cash_flow"), number(r, "market_cap")).map(|x| x * 100.),
        "pct",
        "FCF TTM / capitalización",
    );
    for (key, label) in [
        ("revenue", "Ingresos TTM"),
        ("gross_profit", "Beneficio bruto TTM"),
        ("operating_income", "Beneficio operativo TTM"),
        (
            "net_income_loss_attributable_common_shareholders",
            "Beneficio común TTM",
        ),
        ("ebitda", "EBITDA TTM"),
        ("research_development", "Investigación y desarrollo TTM"),
        ("diluted_earnings_per_share", "BPA diluido reportado TTM"),
    ] {
        add(
            key,
            label,
            "Calidad",
            number(i, key),
            "usd",
            "Estado de resultados TTM",
        );
    }
    for (id, label, num) in [
        ("gross_margin", "Margen bruto", "gross_profit"),
        ("operating_margin", "Margen operativo", "operating_income"),
        (
            "net_margin",
            "Margen neto común",
            "net_income_loss_attributable_common_shareholders",
        ),
    ] {
        add(
            id,
            label,
            "Calidad",
            ratio(number(i, num), number(i, "revenue")).map(|x| x * 100.),
            "pct",
            "Resultado / ingresos TTM",
        );
    }
    let ocf = number(c, "net_cash_from_operating_activities");
    let free = fcf(c);
    add(
        "ocf",
        "Flujo operativo TTM",
        "Caja",
        ocf,
        "usd",
        "Estado de caja TTM",
    );
    add(
        "capex",
        "Inversión en activos CapEx",
        "Caja",
        number(c, "purchase_of_property_plant_and_equipment").map(f64::abs),
        "usd",
        "Salida PPE absoluta TTM",
    );
    add(
        "fcf_computed",
        "Flujo libre calculado",
        "Caja",
        free,
        "usd",
        "OCF − |salidas PPE| TTM",
    );
    add(
        "cash_conversion",
        "Conversión beneficio a caja",
        "Caja",
        if aligned {
            ratio(
                ocf,
                number(i, "net_income_loss_attributable_common_shareholders"),
            )
        } else {
            None
        },
        "x",
        "OCF / beneficio común; mismo período",
    );
    add(
        "fcf_margin",
        "Margen de flujo libre",
        "Caja",
        if aligned {
            ratio(free, number(i, "revenue")).map(|x| x * 100.)
        } else {
            None
        },
        "pct",
        "FCF / ingresos; mismo período",
    );
    add(
        "dividend_cover",
        "Cobertura de dividendos con FCF",
        "Caja",
        ratio(free, number(c, "dividends").map(f64::abs)),
        "x",
        "FCF / |dividendos|; no aplica si no paga",
    );
    let debt = number(b, "debt_current")
        .zip(number(b, "long_term_debt_and_capital_lease_obligations"))
        .map(|(x, y)| x + y);
    let netdebt = debt
        .zip(number(b, "cash_and_equivalents"))
        .map(|(x, y)| x - y);
    for (id, label, val) in [
        ("debt", "Deuda financiera total", debt),
        (
            "cash_balance",
            "Efectivo",
            number(b, "cash_and_equivalents"),
        ),
        ("net_debt", "Deuda neta (sin inversiones)", netdebt),
        ("equity", "Patrimonio", number(b, "total_equity")),
        ("assets", "Activos", number(b, "total_assets")),
        ("liabilities", "Pasivos", number(b, "total_liabilities")),
    ] {
        add(
            id,
            label,
            "Solvencia",
            val,
            "usd",
            "Último balance trimestral",
        );
    }
    add(
        "net_debt_ebitda",
        "Deuda neta / EBITDA",
        "Solvencia",
        if balance_aligned {
            ratio(netdebt, number(i, "ebitda"))
        } else {
            None
        },
        "x",
        "Balance y TTM con mismo cierre",
    );
    add(
        "interest_cover",
        "Cobertura operativa de intereses",
        "Solvencia",
        ratio(number(i, "operating_income"), number(i, "interest_expense")),
        "x",
        "Beneficio operativo / interés TTM",
    );
    add(
        "liabilities_assets",
        "Pasivos / activos",
        "Solvencia",
        ratio(number(b, "total_liabilities"), number(b, "total_assets")).map(|x| x * 100.),
        "pct",
        "Balance trimestral",
    );
    for (id, label, key, n) in [
        ("revenue_yoy", "Crecimiento ingresos anual", "revenue", 1),
        ("revenue_cagr3", "CAGR ingresos 3 años", "revenue", 3),
        (
            "eps_yoy",
            "Crecimiento BPA diluido anual",
            "diluted_earnings_per_share",
            1,
        ),
        (
            "eps_cagr3",
            "CAGR BPA diluido 3 años",
            "diluted_earnings_per_share",
            3,
        ),
    ] {
        add(
            id,
            label,
            "Crecimiento",
            growth(&years, key, n),
            "pct",
            "Ejercicios anuales consecutivos; base positiva",
        );
    }
    let dilution = years
        .first()
        .zip(years.get(1))
        .filter(|(a, b)| {
            a["fiscal_year"]
                .as_i64()
                .zip(b["fiscal_year"].as_i64())
                .is_some_and(|(a, b)| a - b == 1)
        })
        .and_then(|(a, b)| {
            ratio(
                number(a, "diluted_shares_outstanding"),
                number(b, "diluted_shares_outstanding"),
            )
        })
        .map(|x| (x - 1.) * 100.);
    add(
        "dilution",
        "Cambio de acciones diluidas anual",
        "Crecimiento",
        dilution,
        "pct",
        "Promedios anuales; no es recompra exacta",
    );
    let cf_count = cashyears
        .iter()
        .take(5)
        .map(fcf)
        .collect::<Option<Vec<_>>>();
    add(
        "fcf_positive_years",
        "Años con FCF positivo (hasta 5)",
        "Caja",
        cf_count
            .filter(|a| !a.is_empty())
            .map(|a| a.iter().filter(|v| **v > 0.).count() as f64),
        "number",
        "Años disponibles; ver cobertura histórica",
    );
    add(
        "history_years",
        "Ejercicios anuales disponibles",
        "Crecimiento",
        Some(years.len().min(5) as f64),
        "number",
        "Últimos cinco ejercicios reportados",
    );
    let today = crate::calendar::date_at(now)
        .map(|d| d.to_string())
        .unwrap_or_default();
    let mut candles: BTreeMap<u64, Value> = BTreeMap::new();
    for row in values(bars)
        .into_iter()
        .filter(|_| bars["ticker"] == symbol && bars["adjusted"] == true)
    {
        if let Some(t) = row["t"].as_u64() {
            if t <= now
                && now - t <= 1100 * 86_400_000
                && crate::calendar::date_at(t).is_some_and(|d| d.to_string() < today)
                && number(&row, "c").is_some_and(|x| x > 0.)
                && number(&row, "h").is_some_and(|x| x > 0.)
                && number(&row, "l").is_some_and(|x| x > 0.)
            {
                candles.insert(t, row);
            }
        }
    }
    let rows = candles.values().collect::<Vec<_>>();
    let closes = rows
        .iter()
        .filter_map(|r| number(r, "c"))
        .collect::<Vec<_>>();
    let last = closes.last().copied();
    let price_date = candles
        .last_key_value()
        .and_then(|(t, _)| crate::calendar::date_at(*t))
        .map(|d| d.to_string())
        .unwrap_or_default();
    let window = candles
        .iter()
        .filter(|(t, _)| now.saturating_sub(**t) <= 365 * 86_400_000)
        .map(|(_, r)| r)
        .collect::<Vec<_>>();
    let high = if window.len() >= 200 {
        window
            .iter()
            .filter_map(|r| number(r, "h"))
            .reduce(f64::max)
    } else {
        None
    };
    let low = if window.len() >= 200 {
        window
            .iter()
            .filter_map(|r| number(r, "l"))
            .reduce(f64::min)
    } else {
        None
    };
    add(
        "last_close",
        "Último cierre diario completo",
        "Precio",
        last,
        "usd",
        "Velas ajustadas por splits, sin dividendos",
    );
    add(
        "high52",
        "Máximo de 52 semanas",
        "Precio",
        high,
        "usd",
        "Máximos diarios; ≥200 sesiones observadas",
    );
    add(
        "low52",
        "Mínimo de 52 semanas",
        "Precio",
        low,
        "usd",
        "Mínimos diarios; ≥200 sesiones observadas",
    );
    add(
        "drawdown",
        "Caída desde máximo 52 semanas",
        "Precio",
        ratio(last, high).map(|x| (1. - x) * 100.),
        "pct",
        "Caída de precio; no descuento a valor intrínseco",
    );
    for n in [50usize, 200] {
        let sma = if closes.len() >= n {
            Some(closes[closes.len() - n..].iter().sum::<f64>() / n as f64)
        } else {
            None
        };
        add(
            &format!("sma{n}"),
            &format!("Media {n} sesiones"),
            "Precio",
            sma,
            "usd",
            "Cierres ajustados; sesiones observadas",
        );
    }
    for n in [50usize, 200] {
        let value = crate::investment_structure::ema(&closes, n);
        add(
            &format!("ema{n}"),
            &format!("EMA {n} diaria"),
            "Precio",
            value,
            "usd",
            "Semilla SMA y suavizado 2/(n+1); cierres ajustados completos",
        );
        add(
            &format!("ema{n}_distance"),
            &format!("Distancia cierre / EMA {n}"),
            "Precio",
            ratio(last, value).map(|v| (v - 1.) * 100.),
            "pct",
            "Positivo: por encima; negativo: por debajo",
        );
    }
    for (n, label) in [
        (21usize, "1 mes"),
        (63, "3 meses"),
        (126, "6 meses"),
        (252, "12 meses"),
    ] {
        let ret = closes
            .len()
            .checked_sub(n + 1)
            .and_then(|j| ratio(last, Some(closes[j])))
            .map(|x| (x - 1.) * 100.);
        add(
            &format!("return{n}"),
            &format!("Variación {label}"),
            "Precio",
            ret,
            "pct",
            "Aproximación por sesiones; sin dividendos",
        );
    }
    let returns = closes
        .windows(2)
        .rev()
        .take(63)
        .map(|a| (a[1] / a[0]).ln())
        .collect::<Vec<_>>();
    let vol = if returns.len() == 63 {
        let mean = returns.iter().sum::<f64>() / 63.;
        Some(
            (returns.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 62.).sqrt()
                * 252f64.sqrt()
                * 100.,
        )
    } else {
        None
    };
    add(
        "volatility",
        "Volatilidad anualizada 63 sesiones",
        "Precio",
        vol,
        "pct",
        "Desviación retornos log × √252",
    );
    let volume_ratio=if rows.len()>=21 {let recent=&rows[rows.len()-21..rows.len()-1];recent.iter().map(|r|number(r,"v").filter(|v|*v>=0.)).collect::<Option<Vec<_>>>().and_then(|v|{let mean=v.iter().sum::<f64>()/20.;number(rows.last().unwrap(),"v").filter(|v|*v>=0.).and_then(|v|(mean>0.).then_some(v/mean))})}else{None};
    add("volume_ratio20", "Volumen diario / media previa 20 sesiones", "Precio", volume_ratio, "x", "Sesiones cerradas; no sustituye RVOL intradía");
    let g = |id: &str| m.iter().find(|x| x.id == id).and_then(|x| x.value);
    let mut checks = Vec::new();
    let mut check = |label: &str, value: Option<bool>, weight: f64| {
        checks.push(Check {
            label: label.into(),
            passed: value,
            weight,
        })
    };
    check("ROA ≥ 5%", g("return_on_assets").map(|x| x >= 5.), 10.);
    check(
        "Margen operativo ≥ 10%",
        g("operating_margin").map(|x| x >= 10.),
        10.,
    );
    check(
        "Beneficio común positivo",
        g("net_income_loss_attributable_common_shareholders").map(|x| x > 0.),
        10.,
    );
    check("Flujo libre calculado positivo", free.map(|x| x > 0.), 10.);
    check(
        "Conversión a caja ≥ 1x",
        g("cash_conversion").map(|x| x >= 1.),
        5.,
    );
    check(
        "Ingresos anuales crecientes",
        g("revenue_yoy").map(|x| x > 0.),
        10.,
    );
    check(
        "CAGR ingresos 3 años ≥ 5%",
        g("revenue_cagr3").map(|x| x >= 5.),
        5.,
    );
    check("BPA anual creciente", g("eps_yoy").map(|x| x > 0.), 5.);
    check(
        "Deuda neta / EBITDA ≤ 3x",
        g("net_debt_ebitda").map(|x| x <= 3.),
        10.,
    );
    check("Patrimonio positivo", g("equity").map(|x| x > 0.), 5.);
    check("Rendimiento FCF ≥ 4%", g("fcf_yield").map(|x| x >= 4.), 10.);
    check(
        "PER positivo ≤ 25x",
        g("price_to_earnings").map(|x| x > 0. && x <= 25.),
        5.,
    );
    check(
        "Caída supera filtro elegido",
        g("drawdown").map(|x| x >= min_drop),
        5.,
    );
    let score = checks
        .iter()
        .filter(|c| c.passed == Some(true))
        .map(|c| c.weight)
        .sum::<f64>();
    let coverage = checks
        .iter()
        .filter(|c| c.passed.is_some())
        .map(|c| c.weight)
        .sum::<f64>();
    let specialized = text(p, "sic_code")
        .parse::<u32>()
        .ok()
        .is_some_and(|n| (6000..7000).contains(&n));
    let identity = p["ticker"] == symbol
        && p["active"] == true
        && p["type"] == "CS"
        && p["currency_name"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("usd"))
        && !text(p, "sic_code").is_empty();
    let fresh = age(&text(r, "date"), now).is_some_and(|d| d <= 7)
        && age(&price_date, now).is_some_and(|d| d <= 7)
        && [i, c, b]
            .iter()
            .all(|v| age(&text(v, "period_end"), now).is_some_and(|d| d <= 200));
    let critical = g("equity").is_some_and(|v| v <= 0.)
        || free.is_some_and(|v| v <= 0.)
        || g("net_income_loss_attributable_common_shareholders").is_some_and(|v| v <= 0.)
        || g("net_debt_ebitda").is_some_and(|v| v > 5.)
        || g("revenue_yoy").is_some_and(|v| v < -10.);
    if !fresh {
        warnings.push("Fechas ausentes o datos antiguos: no habilita candidatura".into());
    }
    if !identity {
        warnings.push("Identidad, acción común activa en USD o clasificación sin verificar".into());
    }
    if specialized {
        warnings.push("Financiera / seguros / inmobiliaria: exige modelo sectorial; ranking general deshabilitado".into());
    }
    if !aligned || !balance_aligned {
        warnings.push("Períodos financieros no alineados; ratios cruzados sin calcular".into());
    }
    if coverage < 85. {
        warnings
            .push("Cobertura insuficiente para candidatura; ausentes no equivalen a cero".into());
    }
    if g("drawdown").is_some_and(|x| x >= 40.) {
        warnings.push(
            "Caída ≥40%: revisar deterioro del negocio antes de interpretar descuento".into(),
        );
    }
    if critical && !specialized {
        warnings.push(
            "Riesgo fundamental: revisar pérdidas, caja, patrimonio, deuda o caída de ingresos"
                .into(),
        );
    }
    if g("dilution").is_some_and(|x| x > 5.) {
        warnings.push("Dilución anual superior al 5%".into());
    }
    if !specialized && g("interest_cover").is_some_and(|x| x < 2.) {
        warnings.push("Cobertura operativa de intereses inferior a 2x".into());
    }
    if years.len() < 4 {
        warnings.push("Menos de cuatro ejercicios: tendencia de tres años incompleta".into());
    }
    let annual_fresh = years
        .first()
        .is_some_and(|r| age(&text(r, "period_end"), now).is_some_and(|d| d <= 550));
    let required = [
        "equity",
        "net_income_loss_attributable_common_shareholders",
        "fcf_computed",
        "net_debt_ebitda",
        "revenue_yoy",
        "drawdown",
    ]
    .iter()
    .all(|id| g(id).is_some());
    let valuation = g("fcf_yield").is_some_and(|v| v >= 4.)
        || g("price_to_earnings").is_some_and(|v| v > 0. && v <= 25.);
    if !required || !annual_fresh {
        warnings.push("Faltan métricas esenciales o estados anuales recientes".into());
    }
    let eligible = identity && !specialized && fresh && aligned && balance_aligned && annual_fresh;
    let status = if specialized {
        "Especializado"
    } else if !eligible || !required || coverage < 85. {
        "Datos incompletos"
    } else if critical && !specialized {
        "Riesgo elevado"
    } else if g("drawdown").is_some_and(|x| x < min_drop) {
        "Sin caída requerida"
    } else if !valuation {
        "Valoración exigente"
    } else if score >= 75. {
        "Candidata a estudiar"
    } else {
        "Vigilar"
    };
    Company{opportunity:None,ticker:symbol,name:text(p,"name"),industry:text(p,"sic_description"),sic:text(p,"sic_code"),cik:text(p,"cik"),as_of:text(r,"date"),period:text(i,"period_end"),filing:text(i,"filing_date"),price_date,score:if eligible{Some(score)}else{None},coverage,status:status.into(),metrics:m,checks,warnings,annual:years.into_iter().take(5).map(|v|serde_json::json!({"year":v["fiscal_year"],"period":v["period_end"],"filing":v["filing_date"],"revenue":v["revenue"],"netIncome":v["net_income_loss_attributable_common_shareholders"],"eps":v["diluted_earnings_per_share"]})).collect(),closes,source_errors:Vec::new(),eligible,research:None,zones:crate::investment_zones::calculate(&rows)}
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Vec<Value> {
        let now = crate::calendar::rfc3339_ms("2026-09-26T16:00:00Z").unwrap();
        let mut income = vec![
            json!({"tickers":["TEST"],"timeframe":"trailing_twelve_months","period_end":"2026-06-30","filing_date":"2026-08-01","revenue":1000,"operating_income":200,"net_income_loss_attributable_common_shareholders":100,"ebitda":250}),
        ];
        for (year, revenue) in [(2025, 1000), (2024, 900), (2023, 800), (2022, 700)] {
            income.push(json!({"tickers":["TEST"],"timeframe":"annual","fiscal_year":year,"period_end":format!("{year}-12-31"),"filing_date":"2026-08-01","revenue":revenue,"diluted_earnings_per_share":revenue as f64/100.}));
        }
        vec![
            json!({"ticker":"TEST","date":"2026-09-25","return_on_assets":0.1,"price_to_earnings":20,"free_cash_flow":100,"market_cap":1000}),
            json!({"results":{"ticker":"TEST","name":"Test","active":true,"type":"CS","currency_name":"usd","sic_code":"3571"}}),
            json!({"ticker":"TEST","adjusted":true,"results":(1..=260).map(|d|json!({"t":now-d*86_400_000,"c":80,"h":100,"l":70})).collect::<Vec<_>>()}),
            json!({"results":income}),
            json!({"results":[{"tickers":["TEST"],"timeframe":"trailing_twelve_months","period_end":"2026-06-30","filing_date":"2026-08-01","net_cash_from_operating_activities":150,"purchase_of_property_plant_and_equipment":-50}]}),
            json!({"results":[{"tickers":["TEST"],"timeframe":"quarterly","period_end":"2026-06-30","filing_date":"2026-08-01","total_equity":500,"debt_current":50,"long_term_debt_and_capital_lease_obligations":150,"cash_and_equivalents":100}]}),
        ]
    }
    fn evaluate(v: &[Value]) -> Company {
        analyze(Input {
            ratio: &v[0],
            profile: &v[1],
            bars: &v[2],
            income: &v[3],
            cash: &v[4],
            balance: &v[5],
            now: crate::calendar::rfc3339_ms("2026-09-26T16:00:00Z").unwrap(),
            min_drop: 15.,
        })
    }
    #[test]
    fn research_metrics_affect_score_without_bypassing_risk_and_are_idempotent() {
        let mut c = evaluate(&fixture());
        for (id, value) in [
            ("return_on_equity", 20.0),
            ("beta", 1.0),
            ("forward_pe", 20.0),
        ] {
            c.metrics.retain(|m| m.id != id);
            c.metrics.push(Metric {
                id: id.into(),
                label: id.into(),
                group: "test".into(),
                value: Some(value),
                unit: "".into(),
                source: "fixture".into(),
            });
        }
        // A base score near the threshold can be upgraded or downgraded by the new criteria.
        c.checks[0].passed = Some(false);
        c.checks[1].passed = Some(false);
        let mut weak = c.clone();
        for m in &mut weak.metrics {
            if m.id == "return_on_equity" {
                m.value = Some(1.0);
            }
            if m.id == "beta" {
                m.value = Some(3.0);
            }
            if m.id == "forward_pe" {
                m.value = Some(100.0);
            }
        }
        apply_research_metrics(&mut c);
        apply_research_metrics(&mut weak);
        assert_eq!(c.score, Some(84.0));
        assert_eq!(c.status, "Candidata a estudiar");
        assert_eq!(weak.score, Some(64.0));
        assert_eq!(weak.status, "Vigilar");
        assert_eq!(c.checks.iter().map(|c| c.weight).sum::<f64>(), 100.0);
        apply_research_metrics(&mut c);
        assert_eq!(c.score, Some(84.0));
        let mut risk = evaluate(&fixture());
        risk.status = "Riesgo elevado".into();
        apply_research_metrics(&mut risk);
        assert_eq!(risk.status, "Riesgo elevado");
        let mut missing = evaluate(&fixture());
        missing
            .metrics
            .retain(|m| !["return_on_equity", "beta", "forward_pe"].contains(&m.id.as_str()));
        apply_research_metrics(&mut missing);
        assert_eq!(missing.coverage, 80.0);
        assert_eq!(missing.status, "Datos incompletos");
        assert!(missing
            .checks
            .iter()
            .rev()
            .take(3)
            .all(|ch| ch.passed.is_none()));
    }
    #[test]
    fn candidate_requires_both_quality_and_valuation() {
        let mut v = fixture();
        let c = evaluate(&v);
        assert_eq!(c.status, "Candidata a estudiar");
        assert_eq!(c.score, Some(100.));
        v[0]["free_cash_flow"] = json!(10);
        v[0]["price_to_earnings"] = json!(100);
        assert_eq!(evaluate(&v).status, "Valoración exigente");
    }
    #[test]
    fn missing_essential_cash_cannot_pass_with_high_coverage() {
        let mut v = fixture();
        v[4]["results"][0]
            .as_object_mut()
            .unwrap()
            .remove("purchase_of_property_plant_and_equipment");
        let c = evaluate(&v);
        assert!(c.coverage >= 85.);
        assert_eq!(c.status, "Datos incompletos");
    }
    #[test]
    fn negative_cash_and_stale_ratios_are_not_candidates() {
        let mut v = fixture();
        v[4]["results"][0]["net_cash_from_operating_activities"] = json!(-100);
        assert_eq!(evaluate(&v).status, "Riesgo elevado");
        v[0]["date"] = json!("2020-01-01");
        assert_eq!(evaluate(&v).score, None);
    }
    #[test]
    fn specialist_sectors_and_misaligned_periods_have_no_general_score() {
        let mut v = fixture();
        v[1]["results"]["sic_code"] = json!("6021");
        assert_eq!(evaluate(&v).status, "Especializado");
        assert_eq!(evaluate(&v).score, None);
        v[1]["results"]["sic_code"] = json!("3571");
        v[5]["results"][0]["period_end"] = json!("2026-03-31");
        assert_eq!(evaluate(&v).score, None);
    }
    #[test]
    fn incomplete_price_history_never_labels_a_discount() {
        let mut v = fixture();
        v[2]["results"].as_array_mut().unwrap().truncate(50);
        assert_eq!(evaluate(&v).status, "Datos incompletos");
    }
    #[test]
    fn missing_values_never_create_candidate() {
        let empty = json!({});
        let r = json!({"ticker":"TEST"});
        let c = analyze(Input {
            ratio: &r,
            profile: &empty,
            bars: &empty,
            income: &empty,
            cash: &empty,
            balance: &empty,
            now: 1_800_000_000_000,
            min_drop: 15.,
        });
        assert_eq!(c.score, None);
        assert_eq!(c.coverage, 0.);
        assert_eq!(c.status, "Datos incompletos");
    }
    #[test]
    fn capex_is_an_outflow_not_added_twice() {
        assert_eq!(
            fcf(
                &json!({"net_cash_from_operating_activities":100,"purchase_of_property_plant_and_equipment":-20})
            ),
            Some(80.)
        );
        assert_eq!(
            fcf(&json!({"net_cash_from_operating_activities":100})),
            None
        );
    }
    #[test]
    fn growth_requires_consecutive_years_and_positive_base() {
        assert!(growth(
            &[
                json!({"fiscal_year":2025,"revenue":10}),
                json!({"fiscal_year":2023,"revenue":5})
            ],
            "revenue",
            1
        )
        .is_none());
        assert!(growth(
            &[
                json!({"fiscal_year":2025,"revenue":10}),
                json!({"fiscal_year":2024,"revenue":-5})
            ],
            "revenue",
            1
        )
        .is_none());
    }
    #[test]
    fn financials_reject_other_ticker_and_future_filing() {
        let v = json!({"results":[{"tickers":["OTHER"],"timeframe":"annual","period_end":"2025-12-31","filing_date":"2026-02-01"},{"tickers":["TEST"],"timeframe":"annual","period_end":"2025-12-31","filing_date":"2099-02-01"}]});
        assert!(statements(&v, "TEST", "annual", 1_800_000_000_000).is_empty());
    }
}
