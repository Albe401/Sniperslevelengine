//! Long-term opportunity research, adapted from the user-supplied v0.8 scanner.
//! Missing evidence is never replaced with neutral RSI or invented solvency.
use crate::investment::Company;
use serde::{Deserialize, Serialize};
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Opportunity {
 pub version:String,pub at:u64,pub score:Option<f64>,pub coverage:f64,pub blocks:Vec<Block>,
 pub fair_value:Option<f64>,pub fair_low:Option<f64>,pub fair_high:Option<f64>,pub margin:Option<f64>,
 pub scenarios:Vec<Scenario>,pub methods:Vec<Method>,pub assumptions:Vec<String>,
 pub ma200:Option<f64>,pub ma_distance:Option<f64>,pub ma_state:String,
 pub rsi:Option<f64>,pub bull:Vec<String>,pub bear:Vec<String>,pub outlook:String,
 pub entry:Option<f64>,pub invalidation:Option<f64>,pub target:Option<f64>,pub reward_risk:Option<f64>,
 pub analysts:Analysts,pub peer_score:Option<f64>,pub peer_count:usize,pub peer_group:String,
 pub history:Vec<History>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Block {pub name:String,pub score:Option<f64>,pub coverage:f64,pub factors:Vec<Factor>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Factor {pub label:String,pub points:Option<f64>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Scenario {pub name:String,pub growth:f64,pub discount:f64,pub terminal:f64,pub value:Option<f64>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Method {pub name:String,pub value:f64,pub weight:f64}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Analysts {pub source:String,pub status:String,pub fetched_ms:u64,pub provider_date:Option<String>,pub average:Option<f64>,pub low:Option<f64>,pub high:Option<f64>,pub upside:Option<f64>,pub consensus:Option<String>,pub buy:Option<u64>,pub hold:Option<u64>,pub sell:Option<u64>,pub total:Option<u64>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct History {pub at:u64,pub price_date:String,pub score:Option<f64>,pub coverage:f64,pub fair_value:Option<f64>,pub version:String}
pub fn get(c:&Company,id:&str)->Option<f64>{c.metrics.iter().find(|m|m.id==id).and_then(|m|m.value).filter(|v|v.is_finite())}
fn pos(v:Option<f64>)->Option<f64>{v.filter(|v|v.is_finite()&&*v>0.)}
fn linear(v:Option<f64>,bad:f64,good:f64)->Option<f64>{v.map(|v|((v-bad)/(good-bad)*5.).clamp(0.,5.))}
fn band(v:Option<f64>,lo:f64,hi:f64,out_lo:f64,out_hi:f64)->Option<f64>{v.map(|v|if v>=lo&&v<=hi {5.}else if v<lo {if lo==out_lo {0.}else{((v-out_lo)/(lo-out_lo)*5.).clamp(0.,5.)}}else{((out_hi-v)/(out_hi-hi)*5.).clamp(0.,5.)})}
fn block(name:&str,items:Vec<(&str,Option<f64>)>)->Block {
 let count=items.iter().filter(|(_,v)|v.is_some()).count();
 Block{name:name.into(),score:(count>0).then(||items.iter().filter_map(|(_,v)|*v).sum()),coverage:count as f64*20.,factors:items.into_iter().map(|(l,v)|Factor{label:l.into(),points:v}).collect()}
}
fn dcf(fcf:f64,shares:f64,g:f64,r:f64,t:f64)->Option<f64>{
 if fcf<=0.||shares<=0.||r<=t{return None;}
 let mut f=fcf;let mut pv=0.;for year in 1..=5 {f*=1.+g;pv+=f/(1.+r).powi(year);}
 pos(Some((pv+f*(1.+t)/(r-t)/(1.+r).powi(5))/shares))
}
pub fn rsi(closes:&[f64])->Option<f64>{
 if closes.len()<15||closes.iter().any(|v|!v.is_finite()||*v<=0.) {return None;}
 let mut gain=0.;let mut loss=0.;for (i,w) in closes.windows(2).enumerate(){let d=w[1]-w[0];if i<14 {gain+=d.max(0.)/14.;loss+=(-d).max(0.)/14.;}else{gain=(gain*13.+d.max(0.))/14.;loss=(loss*13.+(-d).max(0.))/14.;}}
 if gain+loss==0. {return None;}Some(if loss==0. {100.}else{100.-100./(1.+gain/loss)})
}
pub fn calculate(c:&Company,at:u64)->Opportunity {
 let g=|id| get(c,id);let price=pos(g("last_close"));let shares=pos(g("shares_outstanding"));let fcf=pos(g("fcf_computed"));
 let mut o=Opportunity{version:"opportunity-v1".into(),at,analysts:Analysts{source:"FMP".into(),status:"FMP_API_KEY no configurada".into(),..Default::default()},..Default::default()};
 o.ma200=if c.closes.len()>=200 {pos(Some(c.closes[c.closes.len()-200..].iter().sum::<f64>()/200.))}else{None};
 o.ma_distance=price.zip(o.ma200).map(|(p,m)|(p/m-1.)*100.);
 o.ma_state=match o.ma_distance {Some(v) if v>0.=>"Arriba",Some(v) if v<0.=>"Debajo",Some(_)=>"En la media",None=>"Sin 200 cierres"}.into();o.rsi=rsi(&c.closes);
 o.assumptions=vec!["Modelo de escenarios, no precio objetivo garantizado. Sin calibración histórica.".into(),"DCF a cinco años: crecimiento acotado; tasas de descuento 11%, 9,5% y 8,5%; perpetuidad 2%, 2,5% y 3%.".into(),"OCF−CapEx se usa como aproximación al flujo para el accionista; no se vuelve a sumar caja ni restar deuda. No sustituye un DCF normalizado.".into(),"Combinación 60% DCF, 25% BPA×múltiplo y 15% FCF/acción×múltiplo; pesos redistribuidos solo entre métodos disponibles. Son métodos correlacionados, no confianza estadística.".into()];
 if c.eligible && c.status!="Especializado" {
  let growth=g("revenue_yoy").zip(g("eps_yoy")).map(|(r,e)|(r.min(e)/100.).clamp(-0.02,0.12));
  if let Some(growth)=growth {for (name,growth,discount,terminal) in [("Conservador",(growth-0.04).clamp(-0.03,0.07),0.11,0.02),("Base",growth,0.095,0.025),("Optimista",(growth+0.03).clamp(0.,0.15),0.085,0.03)] {
   o.scenarios.push(Scenario{name:name.into(),growth:growth*100.,discount:discount*100.,terminal:terminal*100.,value:fcf.zip(shares).and_then(|(f,s)|dcf(f,s,growth,discount,terminal))});
  }}
  if let Some(v)=o.scenarios.get(1).and_then(|s|s.value){o.methods.push(Method{name:"DCF aproximado".into(),value:v,weight:0.6});}
  if let Some(((eps,roic),growth))=pos(g("diluted_earnings_per_share")).zip(g("roic")).zip(g("eps_yoy")) {
   if let Some(debt)=g("debt_to_equity").filter(|v|*v>=0.) {let multiple=(16.+((roic-10.)*0.35).clamp(-3.,5.)+((growth-8.)*0.12).clamp(-2.,4.)-if debt>1.5 {2.}else{0.}).clamp(10.,24.);o.methods.push(Method{name:format!("BPA × {multiple:.1}"),value:eps*multiple,weight:0.25});}
  }
  if let Some((((f,s),roic),growth))=fcf.zip(shares).zip(g("roic")).zip(g("revenue_yoy")) {let multiple=(15.+((roic-10.)*0.25).clamp(-2.5,4.)+((growth-7.)*0.1).clamp(-1.5,2.5)).clamp(9.,22.);o.methods.push(Method{name:format!("FCF/acción × {multiple:.1}"),value:f/s*multiple,weight:0.15});}
 }
 o.methods.retain(|m|m.value.is_finite()&&m.value>0.);
 let total_weight:f64=o.methods.iter().map(|m|m.weight).sum();
 if total_weight>0. {o.fair_value=pos(Some(o.methods.iter().map(|m|m.value*m.weight).sum::<f64>()/total_weight));}
 let values:Vec<_>=o.methods.iter().map(|m|m.value).chain(o.scenarios.iter().filter_map(|s|s.value)).filter(|v|v.is_finite()&&*v>0.).collect();
 o.fair_low=values.iter().copied().reduce(f64::min);o.fair_high=values.iter().copied().reduce(f64::max);o.margin=o.fair_value.zip(price).map(|(f,p)|(f-p)/f*100.);
 let margin=g("gross_margin").zip(g("operating_margin")).map(|(a,b)|(a+b)/2.);
 let support=price.and_then(|p|c.zones.supports.iter().filter(|z|z.center<p).max_by(|a,b|a.center.total_cmp(&b.center)));
 let support_quality=support.map(|s|(s.pivots as f64).min(5.));
 let peg=pos(g("peg")).or_else(||pos(g("price_to_earnings")).zip(pos(g("eps_yoy"))).map(|(p,e)|p/e));
 o.blocks=vec![
 block("Calidad del negocio",vec![("Ingresos",linear(g("revenue_yoy"),-5.,15.)),("BPA",linear(g("eps_yoy"),-10.,20.)),("Márgenes",linear(margin,10.,35.)),("Margen FCF",linear(g("fcf_margin"),0.,20.)),("ROIC",linear(g("roic"),5.,20.))]),
 block("Fortaleza financiera",vec![("ROE",linear(g("return_on_equity").filter(|_|g("equity").is_some_and(|v|v>0.)),5.,20.)),("ROIC",linear(g("roic"),5.,20.)),("Deuda/patrimonio",linear(g("debt_to_equity").filter(|v|*v>=0.),2.,0.2)),("Liquidez corriente",linear(g("current"),0.8,2.)),("Cobertura de intereses",linear(g("interest_cover"),1.5,10.))]),
 block("Valoración",vec![("PER",band(pos(g("price_to_earnings")),8.,22.,4.,45.)),("PEG histórico",band(peg,0.7,1.8,0.2,3.5)),("EV/EBITDA",band(pos(g("ev_to_ebitda")),6.,16.,3.,30.)),("P/FCF",band(pos(g("price_to_free_cash_flow")),8.,22.,3.,45.)),("Margen del modelo",linear(o.margin,-10.,30.))]),
 block("Precio y estabilización",vec![("Caída 52 semanas",band(g("drawdown"),15.,35.,0.,55.)),("Soporte: pivotes",support_quality),("Distancia MM200",band(o.ma_distance.map(f64::abs),0.,5.,0.,20.)),("RSI Wilder 14",band(o.rsi,38.,58.,20.,75.)),("Volumen diario relativo",linear(g("volume_ratio20"),0.,2.))])];
 o.coverage=o.blocks.iter().map(|b|b.coverage).sum::<f64>()/4.;
 o.score=(c.eligible&&o.coverage>0.).then(||o.blocks.iter().filter_map(|b|b.score).sum());
 o.bull=c.checks.iter().filter(|x|x.passed==Some(true)).take(4).map(|x|x.label.clone()).collect();
 o.bear=c.checks.iter().filter(|x|x.passed==Some(false)).take(4).map(|x|x.label.clone()).collect();
 o.bear.extend(c.warnings.iter().take(3).cloned());
 o.outlook=format!("{} · {}. Puntuación 4×25 de investigación; cobertura {:.0}%. Conservar las verificaciones fundamentales de Snipers y revisar publicaciones antes de decidir.",c.status,o.ma_state,o.coverage);
 // Price-based research scenario; no arbitrary 8% stop or analyst-target substitution.
 if c.eligible {o.entry=price;o.invalidation=support.map(|s|s.low).filter(|v|*v>0.);
  o.target=price.and_then(|p|c.zones.resistances.iter().filter(|s|s.center>p).min_by(|a,b|a.center.total_cmp(&b.center))).map(|s|s.center);
  o.reward_risk=o.entry.zip(o.invalidation).zip(o.target).and_then(|((p,s),t)|(p>s&&t>p).then_some((t-p)/(p-s)));}
 o
}
#[cfg(test)]mod tests{
 use super::*;use crate::investment::Metric;
 fn sample()->Company {let mut c=Company{ticker:"TEST".into(),industry:"Software".into(),as_of:"2026-09-29".into(),eligible:true,closes:(0..250).map(|i|90.+i as f64/25.).collect(),..Default::default()};
 for (id,value) in [("last_close",100.),("shares_outstanding",100.),("fcf_computed",1000.),("revenue_yoy",10.),("eps_yoy",12.),("roic",18.),("diluted_earnings_per_share",8.),("debt_to_equity",0.5),("return_on_equity",20.),("equity",1000.),("operating_margin",25.),("gross_margin",40.),("fcf_margin",18.),("current",1.5),("interest_cover",8.),("price_to_earnings",18.),("price_to_free_cash_flow",15.),("ev_to_ebitda",10.),("drawdown",20.),("volume_ratio20",1.2)]{c.metrics.push(Metric{id:id.into(),label:id.into(),group:"".into(),unit:"".into(),source:"fixture".into(),value:Some(value)});}c}
 #[test]fn scenarios_ordered_and_margin_uses_fair_value_denominator(){let o=calculate(&sample(),1);assert_eq!(o.methods.len(),3);assert!(o.scenarios[0].value<o.scenarios[1].value&&o.scenarios[1].value<o.scenarios[2].value);let f=o.fair_value.unwrap();assert!((o.margin.unwrap()-(f-100.)/f*100.).abs()<1e-9);assert!(o.reward_risk.is_none());}
 #[test]fn missing_inputs_never_get_neutral_scores(){let o=calculate(&Company::default(),1);assert!(o.score.is_none()&&o.fair_value.is_none()&&o.ma200.is_none()&&o.rsi.is_none());assert_eq!(o.coverage,0.);assert!(o.blocks.iter().all(|b|b.score.is_none()));}
 #[test]fn ma200_requires_exactly_enough_real_closes_and_rsi_is_not_default50(){let mut c=sample();c.closes=vec![100.;199];assert!(calculate(&c,1).ma200.is_none());c.closes.push(100.);assert_eq!(calculate(&c,1).ma200,Some(100.));assert!(rsi(&c.closes).is_none());assert_eq!(rsi(&(1..=30).map(|x|x as f64).collect::<Vec<_>>()),Some(100.));}
 #[test]fn invalid_or_specialized_company_has_no_fair_value(){let mut c=sample();c.eligible=false;assert!(calculate(&c,1).fair_value.is_none());c.eligible=true;c.status="Especializado".into();assert!(calculate(&c,1).fair_value.is_none());}
 #[test]fn more_debt_cannot_improve_financial_score(){let c=sample();let low=calculate(&c,1).blocks[1].score.unwrap();let mut high=c;high.metrics.iter_mut().find(|m|m.id=="debt_to_equity").unwrap().value=Some(4.);assert!(calculate(&high,1).blocks[1].score.unwrap()<low);}
 #[test]fn peers_never_mix_industries_or_score_small_samples(){let mut rows=vec![sample();3];for (i,c) in rows.iter_mut().enumerate(){c.ticker=format!("T{i}");c.opportunity=Some(calculate(c,1));}rows[2].industry="Energy".into();peers(&mut rows);assert!(rows.iter().all(|c|c.opportunity.as_ref().unwrap().peer_score.is_none()));rows[2].industry="Software".into();peers(&mut rows);assert_eq!(rows[0].opportunity.as_ref().unwrap().peer_score,Some(50.));}
}
/// Comparison restricted to same industry and verified companies in this scan.
pub fn peers(rows:&mut [Company]) {
 let metrics=[("return_on_equity",false,0.25),("operating_margin",false,0.20),("revenue_yoy",false,0.20),("debt_to_equity",true,0.20),("price_to_earnings",true,0.15)];
 let source=rows.to_vec();
 for c in rows {let Some(o)=c.opportunity.as_mut() else {continue};let group:Vec<_>=source.iter().filter(|p|p.eligible&&!c.industry.is_empty()&&p.industry==c.industry&&p.as_of==c.as_of).collect();o.peer_group=c.industry.clone();o.peer_count=group.len();o.peer_score=None;
  if !c.eligible||group.len()<3 {continue;}
  let mut score=0.;let mut complete=true;
  for (id,invert,weight) in metrics {let own=c.metrics.iter().find(|m|m.id==id).and_then(|m|m.value).filter(|v|v.is_finite()&&(!invert||*v>0.));let pool:Vec<f64>=group.iter().filter_map(|p|get(p,id)).filter(|v|!invert||*v>0.).collect();
   if let Some(value)=own.filter(|_|pool.len()>=3){let below=pool.iter().filter(|v|**v<value).count() as f64;let equal=pool.iter().filter(|v|**v==value).count() as f64;let pct=(below+equal*0.5)/pool.len() as f64*100.;score+=if invert {(100.-pct)*weight}else{pct*weight};}else{complete=false;}
  }if complete {o.peer_score=Some(score);}
 }
}
