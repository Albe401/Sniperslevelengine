//! Provider schema normalization and conservative event identity. Not primary verification.
use serde_json::{json,Value};
pub fn decorate(item:&mut Value,now:u64,scope:&str){
 item["provider"]=json!("Massive");item["contentSource"]=json!("Benzinga");
 item["verificationStatus"]=json!("REPORTED");item["sourceAuthority"]=json!("NEWS_PROVIDER");
 item["queryScope"]=json!(scope);item["receivedAtMs"]=json!(now);
 let title=item["originalTitle"].as_str().or(item["title"].as_str()).unwrap_or("").to_owned();
 let normalized=title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
 let date=item["publishedAt"].as_str().unwrap_or("").get(..10).unwrap_or("");
 let mut symbols=item["tickerSymbols"].as_array().cloned().unwrap_or_default();symbols.sort_by_key(|v|v.to_string());symbols.dedup();
 item["eventId"]=json!(serde_json::to_string(&json!(["benzinga-exact-headline-v1",date,symbols,normalized])).unwrap_or_default());
 item["eventIdentityMethod"]=json!("same source, exact normalized headline, symbols and publication UTC date; semantic grouping pending");
 let lower=title.to_ascii_lowercase();
 let uncertain=["may ","could ","might ","rumor","not ","no ","potential"].iter().any(|word|lower.contains(word));
 let critical=!uncertain&&["trading halt","halted","trading resumed","bankruptcy","chapter 11","fda approves","fda approval","fda rejects","fda rejection","cuts guidance","withdraws guidance"].iter().any(|word|lower.contains(word));
 item["rawCritical"]=json!(critical);item["processingState"]=json!(if item["translated"]==true{"ENRICHED"}else{"TRANSLATION_PENDING"});
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn exact_identity_keeps_entities_and_numbers(){let mut a=json!({"id":"1","tickerSymbols":["ON"],"title":"Morgan Stanley upgrades ON to $50","publishedAt":"2026-10-13T14:00:00Z"});let mut b=a.clone();b["id"]=json!("2");decorate(&mut a,1,"");decorate(&mut b,1,"");assert_eq!(a["eventId"],b["eventId"]);b["title"]=json!("Morgan Stanley upgrades ON to $60");decorate(&mut b,2,"");assert_ne!(a["eventId"],b["eventId"]);assert_eq!(a["verificationStatus"],"REPORTED");}
 #[test]fn original_critical_is_conservative(){for(t,expected)in[("Trading Halt: Halted at 09:35",true),("Company may declare bankruptcy",false),("FDA approval announced",true)]{let mut v=json!({"title":t});decorate(&mut v,1,"");assert_eq!(v["rawCritical"],expected);}}
}


