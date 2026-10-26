import type {CvdRead} from './cvd';
import type {AuctionReading,AuctionBin} from './auction-map-panel';
import {acceptanceIntensity} from './auction-map-panel';
import type {BidAskWindow} from './strike-bid-ask';
import {barrierFlow} from './barrier-flow';
import {strikeKey} from './strike-key';

export interface LevelActivityInput {
 symbol:string;active:boolean;publishedMs:number|undefined;cvd?:CvdRead;auction?:AuctionReading;
 optionRows?:BidAskWindow[];expirations:string[];optionFresh:boolean;optionAtMs?:number;captured?:boolean;
}
/** value retains source units; intensity is relative display styling, never a new engine score. */
export interface LevelActivityMetric {value:number|null;label:string;detail:string;intensity:number;max:boolean;tied:boolean}
/** Map keys are strikeKey(price), not unnormalized floating point prices. */
export interface LevelActivity {price:number;captured:boolean;acceptance:LevelActivityMetric;stockSales:LevelActivityMetric;optionSales:LevelActivityMetric}
interface StockZone {lower:number;upper:number;firstMs:number;lastMs:number;shares:number;buy:number;sell:number;unknown:number}
interface StockWindow {seconds:number;startMs:number;endMs:number;complete:boolean;classifiedPct?:number|null;excludedZones?:number;zones?:StockZone[]}
interface StockEvidence {version:string;observedSinceMs:number|null;windows:StockWindow[]}
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const clock=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const current=(v:unknown,now:number,age:number):v is number=>clock(v)&&v<=now&&now-v<=age;
const quantity=(v:number)=>v.toLocaleString('en-US',{maximumFractionDigits:3});
const clamp=(v:number)=>Number.isFinite(v)?Math.max(0,Math.min(1,v)):0;
const timeET=(v:number)=>new Date(v).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false});
function disjoint<T>(rows:T[],low:(r:T)=>number,high:(r:T)=>number):boolean {
 const sorted=[...rows].sort((a,b)=>low(a)-low(b));
 return sorted.every((row,i)=>i===0||low(row)>=high(sorted[i-1])-1e-9);
}
function freshStock(input:LevelActivityInput,now:number):boolean {
 const c=input.cvd,last=c?.regular?.lastMs;
 return !!c&&c.symbol===input.symbol&&c.date===dateET.format(now)&&c.available===true&&current(c.at,now,3000)
  &&clock(last)&&last<=now+1000&&now-last<=15000;
}
function stockWindow(input:LevelActivityInput,now:number):StockWindow|undefined {
 if(!freshStock(input,now))return;
 const c=input.cvd!,e=c.executionEvidence as unknown as StockEvidence|undefined;
 if(e?.version!=='execution-evidence-v1'||!Array.isArray(e.windows))return;
 const windows=e.windows.filter(w=>w?.seconds===60);
 if(windows.length!==1)return;
 const w=windows[0];
 if(w.complete!==true||w.endMs!==c.at||w.startMs!==c.at-60000||!clock(e.observedSinceMs)||e.observedSinceMs>w.startMs||!Array.isArray(w.zones)||w.zones.length>100)return;
 if(w.classifiedPct!=null&&(!finite(w.classifiedPct)||w.classifiedPct<0||w.classifiedPct>100))return;
 if(w.excludedZones!=null&&(!Number.isSafeInteger(w.excludedZones)||w.excludedZones<0))return;
 if(w.zones.some(z=>!z||![z.lower,z.upper,z.firstMs,z.lastMs,z.shares,z.buy,z.sell,z.unknown].every(finite)
  ||z.lower<=0||z.upper<=z.lower||Math.abs(z.upper-z.lower-.01)>1e-7||Math.abs(z.lower*100-Math.round(z.lower*100))>1e-6||Math.min(z.shares,z.buy,z.sell,z.unknown)<0
  ||z.firstMs<w.startMs||z.lastMs>w.endMs||z.lastMs<z.firstMs
  ||Math.abs(z.shares-z.buy-z.sell-z.unknown)>Math.max(1e-6,z.shares*1e-9))
  ||!disjoint(w.zones,z=>z.lower,z=>z.upper))return;
 return w;
}
function auctionBins(input:LevelActivityInput,now:number):AuctionBin[]|undefined {
 const r=input.auction;
 // AMT asOf can advance after the last RTH trade: source cadence must remain live too.
 if(!freshStock(input,now)||!r||r.symbol!==input.symbol||r.version!=='auction-map-ticks-v1'
  ||!current(r.asOfMs,now,15000)||dateET.format(r.asOfMs)!==dateET.format(now)
  ||!Number.isSafeInteger(r.acceptedTrades)||r.acceptedTrades<=0||!finite(r.totalVolume)||r.totalVolume<=0
  ||![r.pocLow,r.pocHigh,r.val,r.vah].every(v=>finite(v)&&v>0)||r.pocLow>=r.pocHigh||r.val>=r.vah||!Array.isArray(r.bins))return;
 if(r.bins.some(b=>!b||![b.low,b.high,b.volume,b.brackets,b.volumeRatio,b.timeRatio].every(finite)
  ||b.low<=0||b.high<=b.low||b.volume<0||!Number.isSafeInteger(b.brackets)||b.brackets<0
  ||b.volumeRatio<0||b.volumeRatio>1||b.timeRatio<0||b.timeRatio>1)
  ||!disjoint(r.bins,b=>b.low,b=>b.high))return;
 return r.bins;
}
function normalize(records:LevelActivity[],field:'acceptance'|'stockSales'|'optionSales') {
 const values=records.map(record=>record[field]).filter(metric=>metric.value!=null&&finite(metric.value)&&metric.value>0);
 const maximum=values.length?Math.max(...values.map(metric=>metric.value!)):0;
 const leaders=values.filter(metric=>metric.value===maximum);
 for(const metric of values){metric.intensity=clamp(metric.value!/maximum);metric.max=metric.value===maximum;metric.tied=metric.max&&leaders.length>1;}
}
export function readLevelActivities(prices:number[],input:LevelActivityInput,now:number):Map<number,LevelActivity> {
 const captured=input.captured===true,prefix=captured?'Captura · ':'';
 const context=captured?'Captura identificada; describe la muestra guardada, no actividad actual. ':'';
 const empty=(label:string):LevelActivityMetric=>({value:null,label:prefix+label,detail:context+'Sin evidencia vigente comparable para este nivel.',intensity:0,max:false,tied:false});
 const out=new Map<number,LevelActivity>();
 for(const price of prices){const key=strikeKey(price);if(key!=null&&!out.has(key)){const acceptance=empty('Sin lectura');acceptance.label='Sin lectura';out.set(key,{price:key/1000,captured,acceptance,stockSales:empty('Ventas de acciones'),optionSales:empty('Ventas de opciones')});}}
 if(!clock(now)||(!input.active&&!captured)||!current(input.publishedMs,now,3000))return out;
 const stocks=stockWindow(input,now),bins=auctionBins(input,now);
 const date=dateET.format(now),expirations=Array.isArray(input.expirations)?[...new Set(input.expirations)]:[];
 const optionValid=input.optionFresh===true&&current(input.optionAtMs,now,3000)&&Array.isArray(input.optionRows)&&input.optionRows.every(row=>!!row&&typeof row==='object')&&expirations.length>0
  &&expirations.every(expiration=>/^\d{4}-\d{2}-\d{2}$/.test(expiration)&&expiration>=date);
 for(const record of out.values()) {
  if(stocks){
   const matches=stocks.zones!.filter(z=>record.price>=z.lower&&record.price<z.upper);
   if(matches.length===1){const z=matches[0];record.stockSales.value=z.sell;
    record.stockSales.detail=context+`ACCIONES · BID estimado ${quantity(z.sell)} acciones · precio $${z.lower.toFixed(2)}–$${z.upper.toFixed(2)} (límite superior excluido) · 60 s, ${timeET(stocks.startMs)}–${timeET(stocks.endMs)} ET. Sin clasificar ${quantity(z.unknown)} acciones; clasificación de ventana ${stocks.classifiedPct==null?'no informada':stocks.classifiedPct.toFixed(1)+'%'}; zonas excluidas ${stocks.excludedZones??'no informadas'}. Mayor BID observado solo entre niveles del mapa con muestra; no representa el mercado completo ni órdenes del strike.`;}
  }
  if(bins){
   const matches=bins.filter(b=>record.price>=b.low&&record.price<b.high),b=matches.length===1?matches[0]:undefined;
   const intensity=acceptanceIntensity(b);
   if(b&&intensity){record.acceptance.value=intensity.score;record.acceptance.label=intensity.label;
    record.acceptance.detail=context+`AMT · zona del subyacente $${quantity(b.low)}–$${quantity(b.high)} · perfil ${input.auction!.provisional?'parcial':'observado'} a ${timeET(input.auction!.asOfMs)} ET. Volumen ${quantity(b.volume)} acciones; presencia en ${b.brackets} intervalos. Índice de aceptación ya existente del perfil; no es probabilidad ni flujo de opciones.`;}
  }
  if(optionValid){
   const flow=barrierFlow(input.optionRows,record.price,expirations,true);
   if(flow){record.optionSales.value=flow.sell;
    record.optionSales.detail=context+`OPCIONES · BID estimado ${quantity(flow.sell)} contratos · strike exacto $${quantity(record.price)} · 5 min · ${expirations.join(', ')} · lectura ${timeET(input.optionAtMs!)} ET. Sin clasificar ${quantity(flow.unknown)} contratos; cobertura clasificada ${flow.coverage.toFixed(1)}%. Muestra suscrita con ambos lados y todos los vencimientos seleccionados. Mayor BID observado solo entre niveles del mapa; PUT no significa venta, ni BID identifica apertura/cierre o inventario.`;}
  }
 }
 const records=[...out.values()];
 for(const field of ['acceptance','stockSales','optionSales'] as const)normalize(records,field);
 return out;
}
