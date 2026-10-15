//! Shared stock quantity rule. Options remain measured in contracts.
//! Original decimal text stays in TradeMetadata / the raw recording.
pub fn stock_trade_size(legacy: f64, decimal: Option<&str>) -> Option<f64> {
    let size = match decimal { Some(value) => value.parse::<f64>().ok()?, None => legacy };
    (size.is_finite() && size > 0.).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authoritative_decimal_preserves_fractional_shares() {
        assert_eq!(stock_trade_size(0., Some("0.002000")), Some(0.002));
        assert_eq!(stock_trade_size(100., Some("100.5")), Some(100.5));
        assert_eq!(stock_trade_size(10., None), Some(10.));
        for value in ["NaN", "inf", "-1", "0", "bad", ""] {
            assert_eq!(stock_trade_size(100., Some(value)), None);
        }
        for legacy in [0., -1., f64::NAN, f64::INFINITY] {
            assert_eq!(stock_trade_size(legacy, None), None);
        }
    }
}
