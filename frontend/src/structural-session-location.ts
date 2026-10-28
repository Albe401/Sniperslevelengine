import type {LiveState} from './live';
import {currentSessionCandles} from './candle-series';
import {publicationCandleClock} from './publication-candle-clock';
import {sessionRouteLevels} from './session-intraday-route';
import {longStructureBars} from './price-structure-long';
import {priceStructureBars,type FiveMinuteStructureCard} from './price-structure';
import type {PriceStructureHistory} from './price-structure-history';
import {escapeHtml as esc} from './model';
export interface SessionLocation {
 version:'structural-session-location-v1';symbol:string;observedMs:number;publishedMs:number|null;availability:'current'|'reference'|'pending';
 price:number|null;priceAtMs:number|null;basis:'closed-5m';atr:number|null;
 opening:{high:number;low:number;closedAtMs:number;position:number|null}|null;
 previous:{date:string;high:number;low:number;close:number;closedAtMs:number;source:'official-daily'|'complete-RTH-minutes'}|null;
 vwap:{value:number;atMs:number;source:'provider-minute-vwap'|'existing-session-metrics';difference:number;distanceAtr:number|null;relation:'SOBRE'|'BAJO'|'EN'}|null;
 nearestAbove:{name:string;price:number}|null;nearestBelow:{name:string;price:number}|null;
 reason:string;probability:null;entryAuthorized:false;
}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const positive=(v:unknown):v is number=>finite(v)&&v>0;
const clockValue=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const ratio=(a:number,b:number):number|null=>finite(a/b)?a/b:null;
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Existing source measurements only, all compared at the same closed5mcut. No typical-price VWAP or entry gate. */
export function readSessionLocation(s:LiveState|undefined,symbol:string,active:boolean,now:number,five:FiveMinuteStructureCard,longHistory?:PriceStructureHistory):SessionLocation {
 const out:SessionLocation={version:'structural-session-location-v1',symbol,observedMs:now,publishedMs:s?.publishedMs??null,availability:'pending',price:null,priceAtMs:null,basis:'closed-5m',atr:null,opening:null,previous:null,vwap:null,nearestAbove:null,nearestBelow:null,reason:'Esperando un cierre de5m asociado y verificable.',probability:null,entryAuthorized:false};
 if(!clockValue(now)||!active||s?.symbol!==symbol||!clockValue(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>15000||five.sourceSymbol!==symbol||five.sourcePublishedMs!==s.publishedMs||!positive(five.lastClose)||!clockValue(five.endMs)||five.endMs>s.publishedMs)return out;
 const clock=publicationCandleClock(s.publishedMs,now),date=dateET.format(now),session=s.chartHistory?.sessions.find(v=>v.date===date);
 out.price=five.lastClose;out.priceAtMs=five.endMs;out.atr=positive(five.atr)?five.atr:null;
 out.availability=s.metrics?.phase==='Sesión RTH'&&s.stocks?.phase==='streaming'&&session&&now>=session.openMs&&now<session.closeMs&&dateET.format(five.endMs)===date?'current':'reference';
 const refs:{name:string;price:number}[]=[];
 // Reuse the exact opening-range helper used by the one-minute route.
 const levels=sessionRouteLevels(s,now),high=levels.find(v=>v.name==='RANGO APERTURA · ALTO'),low=levels.find(v=>v.name==='RANGO APERTURA · BAJO');
 if(session&&session.openMs+900000<=five.endMs&&high&&low&&positive(high.price)&&positive(low.price)&&high.price>=low.price){
  out.opening={high:high.price,low:low.price,closedAtMs:session.openMs+900000,position:high.price>low.price?ratio(out.price-low.price,high.price-low.price):null};
  refs.push({name:'Rango apertura · alto',price:high.price},{name:'Rango apertura · bajo',price:low.price});
 }
 const official=longStructureBars(longHistory,symbol,1440,now).bars.filter(b=>b.sessionDate<date&&b.endMs<=clock).at(-1);
 if(official)out.previous={date:official.sessionDate,high:official.high,low:official.low,close:official.close,closedAtMs:official.endMs,source:'official-daily'};
 else if(s.chartHistory){
  const sessions=s.chartHistory.sessions.filter(v=>v.date<date&&v.closeMs<=clock),expected=sessions.at(-1);
  if(expected){const past={sessions,minutes:s.chartHistory.minutes.filter(b=>b.timestampMs<expected.closeMs)},bar=priceStructureBars(past,1440,clock).bars.at(-1);if(bar?.sessionDate===expected.date)out.previous={date:bar.sessionDate,high:bar.high,low:bar.low,close:bar.close,closedAtMs:bar.endMs,source:'complete-RTH-minutes'};}
 }
 if(out.previous)refs.push({name:'Sesión anterior · alto',price:out.previous.high},{name:'Sesión anterior · bajo',price:out.previous.low},{name:'Sesión anterior · cierre',price:out.previous.close});
 // sessionVwap uses actual provider minuteVWAP×volume and becomes null on missing coverage.
 const bar=currentSessionCandles(s.chartHistory,5,clock).find(b=>b.complete&&b.timestampMs+300000===five.endMs&&b.close===five.lastClose);
 let v=positive(bar?.sessionVwap)?bar!.sessionVwap:null,source:NonNullable<SessionLocation['vwap']>['source']='provider-minute-vwap';
 // Metrics are a fallback only at exactly the same economic close, never a later VWAP.
 if(v==null&&s.metrics?.session===date&&s.metrics.asofMs===five.endMs&&positive(s.metrics.vwap)){v=s.metrics.vwap;source='existing-session-metrics';}
 if(positive(v)){const difference=out.price-v;out.vwap={value:v,atMs:five.endMs,source,difference,distanceAtr:out.atr==null?null:ratio(difference,out.atr),relation:difference>0?'SOBRE':difference<0?'BAJO':'EN'};refs.push({name:'VWAP al cierre de5m',price:v});}
 out.nearestAbove=refs.filter(v=>v.price>out.price!).sort((a,b)=>a.price-b.price)[0]??null;
 out.nearestBelow=refs.filter(v=>v.price<out.price!).sort((a,b)=>b.price-a.price)[0]??null;
 out.reason='Ubicación del cierre de5m. '+(out.opening?'Rango inicial cerrado. ':'Rango inicial pendiente. ')+(out.previous?'Sesión anterior identificada. ':'Sesión anterior sin cobertura completa. ')+(out.vwap?'VWAP del proveedor asociado al mismo cierre.':'VWAP comparable pendiente; no se inventa con OHLC.');
 return out;
}
const usd=(n:number|null)=>n==null?'—':'$'+n.toFixed(2);
const stamp=(n:number|null)=>n==null?'sin fecha':new Date(n).toLocaleString('es',{timeZone:'America/New_York',hour12:false})+' ET';
export function sessionLocationDetails(r:SessionLocation):string {
 return `<details data-horizon-details="structural-session-location" data-session-location="${r.availability}"><summary>Ubicación en la sesión · apertura, VWAP y referencias previas</summary><p>${esc(r.reason)} ${r.availability==='reference'?'Referencia histórica.':''} Precio ${usd(r.price)} · cierre ${esc(stamp(r.priceAtMs))}.</p><p>Rango inicial15m: ${r.opening?usd(r.opening.low)+' — '+usd(r.opening.high)+'; cierre '+esc(stamp(r.opening.closedAtMs))+'; posición '+(r.opening.position==null?'sin amplitud':r.opening.position.toFixed(4)+' del rango, sin recortar fuera de0–1'):'pendiente de15minutos consecutivos y cerrados'}.</p><p>Sesión anterior: ${r.previous?esc(r.previous.date)+' · máximo '+usd(r.previous.high)+' · mínimo '+usd(r.previous.low)+' · cierre '+usd(r.previous.close)+' · '+esc(r.previous.source):'sin referencia completa'}.</p><p>VWAP: ${r.vwap?usd(r.vwap.value)+' · '+r.vwap.relation+' · distancia '+r.vwap.difference.toFixed(4)+' USD'+(r.vwap.distanceAtr==null?'':' / '+r.vwap.distanceAtr.toFixed(4)+' ATR5m')+' · '+esc(stamp(r.vwap.atMs))+' · '+esc(r.vwap.source):'pendiente; no se sustituye con precio típico de velas'}.</p><p>Referencia superior: ${r.nearestAbove?esc(r.nearestAbove.name)+' '+usd(r.nearestAbove.price):'sin referencia observada'}. Inferior: ${r.nearestBelow?esc(r.nearestBelow.name)+' '+usd(r.nearestBelow.price):'sin referencia observada'}. Son referencias conocidas, no objetivos garantizados ni condiciones nuevas de entrada.</p></details>`;
}
