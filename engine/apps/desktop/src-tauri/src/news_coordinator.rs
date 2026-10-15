use massive_feed::investment::NewsReport;
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Default)]pub struct NewsCoordinator {pub reports:BTreeMap<String,NewsReport>,pub running:bool}
pub fn batch(cache:&NewsCoordinator)->Vec<Value>{
 let mut unique=BTreeMap::new();
 for item in cache.reports.values().flat_map(|r|r.items.iter()).filter(|n|n["translated"]!=true){unique.insert(serde_json::json!([item["id"],item["title"],item["summary"],item["providerUpdatedAt"]]).to_string(),item.clone());}
 let mut items=unique.into_values().collect::<Vec<_>>();
 items.sort_by(|a,b|(b["rawCritical"]==true).cmp(&(a["rawCritical"]==true)).then_with(||b["publishedAt"].as_str().cmp(&a["publishedAt"].as_str())));
 items.truncate(32);items
}
pub fn merge(cache:&mut NewsCoordinator,translated:&[Value]) {
 for report in cache.reports.values_mut(){for item in &mut report.items {if let Some(done)=translated.iter().find(|n|n["id"]==item["id"]&&n["originalTitle"]==item["title"]&&n["originalSummary"]==item["summary"]&&n["providerUpdatedAt"]==item["providerUpdatedAt"]){*item=done.clone();}}}
}
#[cfg(test)]mod tests{use super::*;use serde_json::json;
 #[test]fn shared_articles_get_one_bounded_batch_and_revisions_do_not_get_old_translation(){let mut c=NewsCoordinator::default();for key in ["","QQQ"]{c.reports.insert(key.into(),NewsReport{items:vec![json!({"id":"1","title":"Old","summary":"Text"})],fetched_ms:1,truncated:false});}assert_eq!(batch(&c).len(),1);c.reports.get_mut("QQQ").unwrap().items[0]["title"]=json!("New");merge(&mut c,&[json!({"id":"1","originalTitle":"Old","originalSummary":"Text","title":"Traducida","translated":true})]);assert_eq!(c.reports["QQQ"].items[0]["title"],"New");assert_eq!(c.reports[""].items[0]["translated"],true);}
}


