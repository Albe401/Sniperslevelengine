import type {LiveState} from './live';
import type {AtrRead} from './intraday-atr';
import type {MarketEvidence,PressureEvidence} from './market-evidence';
import {escapeHtml as esc} from './model';
import './price-extension.css';
export interface PriceExtension {version:'price-extension-v1';symbol:string;state:'pending'|'normal'|'extended'|'extreme';side:'upper'|'lower'|'center';support:'pending'|'mixed'|'strong'|'weakening';distance:number|null;range:number|null;near:string|null;detail:string;supportReason?:string}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const positive=(v:unknown):v is number=>finite(v)&&v>0;
const fresh=(at:unknown,now:number,limit:number)=>finite(at)&&at<=now&&now-at<=limit;
const dates=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Descriptive, provisional thresholds; extension alone never signals a reversal or an order. */
export function priceExtension(s:LiveState|undefined,symbol:string,active:boolean,atr:AtrRead,market:MarketEvidence|undefined,now=Date.now()):PriceExtension {
 const out:PriceExtension={version:'price-extension-v1',symbol,state:'pending',side:'center',support:'pending',distance:null,range:null,near:null,detail:'Esperando precio, VWAP RTH y ATR 5 min vigentes. Extensión no equivale a giro.'};
 if(!finite(now)||!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||!fresh(s.publishedMs,now,3000))return out;
 const today=dates.format(now),session=s.chartHistory?.sessions.find(v=>v.date===today),tick=s.ticks?.[symbol];
 if(!session||!finite(session.openMs)||!finite(session.closeMs)||now<session.openMs||now>=session.closeMs||s.metrics?.session!==today||s.metrics.phase!=='Sesión RTH'||tick?.symbol!==symbol||!positive(tick.price)||!fresh(tick.tradeMs,now,15000)||!positive(s.metrics.vwap)||!fresh(s.metrics.asofMs,now,180000)||!positive(atr.value)||!fresh(atr.endMs,now,300000)||atr.endMs!<session.openMs||dates.format(atr.endMs!)!==today)return out;
 const spot=tick.price; const signed=(spot-s.metrics.vwap)/atr.value,distance=Math.abs(signed);
 if(!finite(distance))return out;
 out.distance=distance;out.side=signed>0?'upper':signed<0?'lower':'center';out.state=distance>=3?'extreme':distance>=2?'extended':'normal';
 const previous=s.chartHistory?.sessions.filter(v=>v.closeMs<now).at(-1)?.date;
 if(atr.daily?.symbol===symbol&&atr.daily.through===previous&&positive(atr.daily.value)&&finite(atr.context?.rthRangeDaily)&&atr.context!.rthRangeDaily!>=0)out.range=atr.context!.rthRangeDaily!;
 // MarketEvidence.session includes asset, ET date and connection generation.
 // Comparing it with the date alone silently rejected every real reading.
 const same=market?.symbol===symbol&&market.session===`${symbol}:${today}:${s.startedMs??0}`&&fresh(market.at,now,3000),sign=Math.sign(signed);
 const pressure=(p:PressureEvidence|undefined)=>same&&p&&fresh(p.at,now,3000)&&finite(p.signal)&&Math.abs(p.signal)<=1&&finite(p.coverage)&&p.coverage>=70&&p.coverage<=100?Math.sign(p.signal)*sign:null;
 const cvd=pressure(market?.cvd),flow=pressure(market?.flow);
 const end=Math.floor(now/60000)*60000,start=end-11*60000;
 const rows=(s.chartHistory?.minutes??[]).filter(v=>v.timestampMs>=start&&v.timestampMs<end).sort((a,b)=>a.timestampMs-b.timestampMs);
 const complete=start>=session.openMs&&rows.length===11&&rows.every((v,i)=>v.timestampMs===start+i*60000&&[v.open,v.high,v.low,v.close,v.volume].every(finite)&&v.low>0&&v.volume>=0&&v.high>=Math.max(v.open,v.close)&&v.low<=Math.min(v.open,v.close));
 if(sign&&complete&&cvd!=null&&flow!=null){
  const before=(rows[5].close-rows[0].close)*sign,recent=(rows[10].close-rows[5].close)*sign;
  out.support=out.state!=='normal'&&before>0&&recent<=before*.5&&cvd<0&&flow<0?'weakening':recent>0&&cvd>0&&flow>0?'strong':'mixed';
 }
 out.supportReason=!same?'Evidencia de otra sesión, conexión o sin vigencia':!complete?'Faltan 11 minutos cerrados consecutivos':!sign?'Precio sobre el VWAP: sin extensión direccional':cvd==null?`CVD sin confirmación: ${market?.cvd.reason??'pendiente'} (vigencia y cobertura ≥70%)`:flow==null?`Prima sin confirmación: ${market?.flow.reason??'pendiente'} (vigencia y cobertura ≥70%)`:'Precio, CVD y prima comparables; extensión no confirma giro';
 if(same){const nearest=market!.levels.filter(l=>positive(l.price)&&l.expiration===today&&fresh(l.asof??l.observed,now,900000)&&Math.abs(l.price-spot)<=atr.value!*.25).sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot))[0];if(nearest)out.near=`${nearest.name} $${nearest.price.toFixed(2)}${nearest.provisional?' · provisional':''}`;}
 out.detail='Distancia absoluta al VWAP RTH / ATR 5 min: normal <2, extendido ≥2, extremo ≥3. Umbrales iniciales descriptivos, pendientes de calibración histórica. Rango diario no es un límite ni rango restante. Agotamiento posible requiere menor avance en dos tramos cerrados de 5 min y CVD + prima de opciones contrarios con ≥70% de cobertura; no confirma giro. Sin datos suficientes, confirmación pendiente. No cambia porcentajes ni órdenes.';
 return out;
}
export function priceExtensionPanel(r:PriceExtension):string {
 const labels={pending:'PENDIENTE',normal:'NORMAL',extended:'EXTENDIDO',extreme:'ZONA EXTREMA'},support={pending:'Confirmación pendiente',mixed:'Apoyo mixto',strong:'Con fuerza',weakening:'Posible agotamiento'};
 return `<section class="price-extension ${r.state}" aria-label="Extensión del precio"><header><strong>EXTENSIÓN DEL PRECIO</strong><b>${labels[r.state]}</b></header><div class="extension-facts"><span>${r.side==='upper'?'Sobre VWAP':r.side==='lower'?'Bajo VWAP':'Distancia al VWAP'} <b>${r.distance==null?'—':r.distance.toFixed(2)+' ATR'}</b></span><span>Rango del día <b>${r.range==null?'—':r.range.toFixed(2)+' ATR D'}</b></span><strong class="${r.support}" title="${esc(r.supportReason??r.detail)}">${support[r.support]}</strong></div>${r.near?`<small>En prueba: ${esc(r.near)}</small>`:''}<details data-horizon-details="price-extension"><summary>Cómo leerlo</summary>${r.supportReason?`<p>${esc(r.supportReason)}</p>`:''}<p>${esc(r.detail)}</p></details></section>`;
}
