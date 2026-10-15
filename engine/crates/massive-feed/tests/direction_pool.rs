use massive_feed::live::direction_pool_symbols;
#[test]
fn disabled_preserves_membership(){let old=vec!["AMD".into(),"SPY".into()];assert_eq!(direction_pool_symbols(old.clone(),"QQQ",false),old);}
#[test]
fn adds_only_priorities_bounded_and_deduplicated(){let result=direction_pool_symbols(vec!["SPY".into()],"QQQ",true);assert!(result.len()<=10);assert!(!result.iter().any(|s|s=="QQQ"));assert_eq!(result.iter().filter(|s|*s=="SPY").count(),1);assert_eq!(result[1],"NVDA");}
#[test]
fn does_not_displace_portfolio_members(){let old:Vec<_>=["AMD","NVDA","TSLA","QQQ","SPY","AAPL","MSFT","META","AMZN","GOOGL"].into_iter().map(str::to_owned).collect();assert_eq!(direction_pool_symbols(old.clone(),"QQQ",true),old);}
