import {longStructureBars} from './price-structure-long';
import {mergeStructureMinutes,type PriceStructureHistory} from './price-structure-history';
import type {Candle} from './model';
import type {ChartHistory} from './candle-series';
import type {LiveState} from './live';

/** Descriptive project convention; no fitted probability or trading authorization. */
export const PRICE_STRUCTURE_RULE = Object.freeze({atrPeriod:14,pivotLeft:2,pivotRight:2,pivotLookback:100,thresholdAtr:0,minimumBars:100,seedTolerance:.005});
export type PriceInterval=15|60|240|1440|10080;
export const PRICE_INTERVALS=[15,60,240,1440,10080] as const;
const names:Record<PriceInterval,[string,string]>={15:['15 MINUTOS','CERCANA'],60:['1 HORA','INTERMEDIA'],240:['4 HORAS','AMPLIA'],1440:['DIARIO','PRINCIPAL'],10080:['SEMANAL','MAYOR']};
export type PriceStructureSide = 'CALL'|'PUT'|'MIXTA'|'SIN DATOS';
export interface PriceBar extends Candle {endMs:number;sessionDate:string}
export interface PricePivot {kind:'high'|'low';price:number;index:number;at:number;confirmedMs:number}
export interface PriceStructureCard {
 interval:PriceInterval;name:string;role:string;side:PriceStructureSide;reason:string;
 bars:number;atr:number|null;highChange:number|null;lowChange:number|null;
 highs:PricePivot[];lows:PricePivot[];lastClose:number|null;endMs:number|null;
 reference:boolean;gaps:number;invalidated:boolean;lastDuration:number|null;seedDifference:number|null;
}
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const validDate=(s:string)=>/^\d{4}-\d{2}-\d{2}$/.test(s)&&Number.isFinite(Date.parse(s+'T12:00:00Z'))&&new Date(s+'T12:00:00Z').toISOString().slice(0,10)===s;
export interface PriceBarSeries {bars:PriceBar[];gaps:number;reason:string}

/** Canonical RTH minutes only. Missing closed buckets reset the sequence; no gap filling.
 * Intraday buckets start at the session open; the final session bucket closes at the actual exchange close.
 * A daily candle requires every official minute of its actual session, including half days.
 */
export function priceStructureBars(history:ChartHistory|undefined,interval:15|60|240|1440,now:number):PriceBarSeries {
 const fail=(reason:string):PriceBarSeries=>({bars:[],gaps:0,reason});
 if(!history||!finite(now)||!Array.isArray(history.minutes)||!Array.isArray(history.sessions))return fail('Sin historial RTH verificable.');
 const sessions=[...history.sessions].sort((a,b)=>a.openMs-b.openMs);
 if(sessions.some((s,i)=>!validDate(s.date)||![s.openMs,s.closeMs].every(finite)||s.openMs%60000!==0||s.closeMs%60000!==0||s.closeMs<=s.openMs||s.closeMs-s.openMs>86400000||day.format(s.openMs)!==s.date||i>0&&(s.openMs<sessions[i-1].closeMs||s.date===sessions[i-1].date)))return fail('Calendario de sesiones inválido o duplicado.');
 const minutes=new Map<number,Candle>();
 for(const b of history.minutes){
  if(!finite(b.timestampMs))return fail('Minuto sin fecha válida.');
  if(b.timestampMs+60000>now)continue;
  if(![b.open,b.high,b.low,b.close,b.volume].every(finite)||b.timestampMs%60000!==0||Math.min(b.open,b.low,b.close)<=0||b.high<Math.max(b.open,b.close,b.low)||b.low>Math.min(b.open,b.close)||b.volume<0||minutes.has(b.timestampMs))return fail('Minutos inválidos o duplicados; sin sustituciones.');
  minutes.set(b.timestampMs,b);
 }
 let bars:PriceBar[]=[],gaps=0;
 for(const session of sessions){
  if(session.openMs>now)continue;
  const duration=interval===1440?session.closeMs-session.openMs:interval*60000;
  for(let start=session.openMs;start<session.closeMs;start+=duration){
   const end=Math.min(start+duration,session.closeMs);
   if(end>now)break;
   let open=0,high=-Infinity,low=Infinity,close=0,volume=0,complete=true;
   for(let t=start;t<end;t+=60000){const b=minutes.get(t);if(!b){complete=false;break;}if(t===start)open=b.open;high=Math.max(high,b.high);low=Math.min(low,b.low);close=b.close;volume+=b.volume;}
   if(!complete||!finite(volume)){gaps++;bars=[];continue;}
   bars.push({timestampMs:start,endMs:end,sessionDate:session.date,open,high,low,close,volume});
  }
 }
 return {bars,gaps,reason:bars.length?'':gaps?'Faltan minutos en la última vela cerrada.':'Sin velas completas de esta temporalidad.'};
}

/** Wilder ATR14, seeded by 14 true ranges with real preceding closes. */
export function priceStructureAtr(bars:PriceBar[]):number|null {
 const n=PRICE_STRUCTURE_RULE.atrPeriod;
 if(bars.length<n+1)return null;
 let atr=0;
 for(let i=1;i<bars.length;i++){
  const b=bars[i],previous=bars[i-1].close;
  const tr=Math.max(b.high-b.low,Math.abs(b.high-previous),Math.abs(b.low-previous));
  if(!finite(tr)||tr<0)return null;
  if(i<=n){atr+=tr/n;}else atr=(atr*(n-1)+tr)/n;
 }
 return finite(atr)&&atr>0?atr:null;
}

/** Strict extrema, two closed bars on either side. Consecutive same-kind extrema collapse.
 * The pivot is available only at confirmedMs; its historical date is never its signal time.
 */
export function priceStructurePivots(bars:PriceBar[]):PricePivot[] {
 const {pivotLeft:left,pivotRight:right,pivotLookback:lookback}=PRICE_STRUCTURE_RULE;
 const pivots:PricePivot[]=[];
 for(let i=Math.max(left,bars.length-lookback);i<bars.length-right;i++){
  const neighbours=bars.slice(i-left,i+right+1).filter((_,j)=>j!==left);
  const high=neighbours.every(b=>bars[i].high>b.high),low=neighbours.every(b=>bars[i].low<b.low);
  if(high===low)continue; // Ties and outside bars do not establish an unambiguous turning point.
  const pivot:PricePivot={kind:high?'high':'low',price:high?bars[i].high:bars[i].low,index:i,at:bars[i].timestampMs,confirmedMs:bars[i+right].endMs};
  const last=pivots.at(-1);
  if(last?.kind===pivot.kind){if(pivot.kind==='high'?pivot.price>last.price:pivot.price<last.price)pivots[pivots.length-1]=pivot;}
  else pivots.push(pivot);
 }
 return pivots;
}

export function classifyPriceStructure(series:PriceBarSeries,interval:PriceInterval,now:number):PriceStructureCard {
 const bars=series.bars,last=bars.at(-1),atr=priceStructureAtr(bars),pivots=priceStructurePivots(bars);
 const highs=pivots.filter(p=>p.kind==='high').slice(-2),lows=pivots.filter(p=>p.kind==='low').slice(-2);
 const card:PriceStructureCard={interval,name:names[interval][0],role:names[interval][1],side:'SIN DATOS',reason:series.reason,bars:bars.length,atr,highChange:null,lowChange:null,highs,lows,lastClose:last?.close??null,endMs:last?.endMs??null,reference:!!last&&day.format(last.endMs)!==day.format(now),gaps:series.gaps,invalidated:false,seedDifference:null,lastDuration:last?(last.endMs-last.timestampMs)/60000:null};
 if(!last||atr==null){card.reason ||= 'Se necesitan 15 velas cerradas continuas para ATR14.';return card;}
 if(highs.length<2||lows.length<2){card.reason='Faltan dos máximos y dos mínimos confirmados en las últimas 100 velas.';return card;}
 card.highChange=(highs[1].price-highs[0].price)/atr;
 card.lowChange=(lows[1].price-lows[0].price)/atr;
 const tau=PRICE_STRUCTURE_RULE.thresholdAtr;
 const call=card.highChange>tau&&card.lowChange>tau,put=card.highChange< -tau&&card.lowChange< -tau;
 const after=Math.max(highs[1].index,lows[1].index)+1;
 card.invalidated=call?bars.slice(after).some(b=>b.close<lows[1].price-tau*atr):put?bars.slice(after).some(b=>b.close>highs[1].price+tau*atr):false;
 card.side=card.invalidated?'MIXTA':call?'CALL':put?'PUT':'MIXTA';
 card.reason=card.invalidated?'Un cierre rompió el último extremo de soporte de la estructura.':call?'Máximos y mínimos confirmados ascendentes.':put?'Máximos y mínimos confirmados descendentes.':'Los máximos y mínimos no definen la misma dirección.';
 return card;
}

export function priceStructureCards(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),history:ChartHistory|undefined=s?.chartHistory,longHistory?:PriceStructureHistory):PriceStructureCard[] {
 const own=active&&s?.symbol===symbol&&finite(s.publishedMs)&&s.publishedMs<=now&&now-s.publishedMs<=15000;
 const minutes=longHistory?.symbol===symbol&&longHistory.asOfMs<=now&&day.format(longHistory.asOfMs)===day.format(now)?mergeStructureMinutes(longHistory.intraday,s?.chartHistory):history;
 return PRICE_INTERVALS.map(interval=>{
  if(!own)return classifyPriceStructure({bars:[],gaps:0,reason:!active?'Data desconectada.':s?.symbol!==symbol?'Historial de otro activo.':'Publicación sin vigencia.'},interval,now);
  const series=interval===1440||interval===10080?longStructureBars(longHistory,symbol,interval,now):priceStructureBars(minutes,interval,now);
  const card=classifyPriceStructure(series,interval,now);
  if(series.bars.length>0&&series.bars.length<PRICE_STRUCTURE_RULE.minimumBars){card.side='SIN DATOS';card.reason='Estabilizando historial: '+series.bars.length+'/100 velas cerradas.';}
  if(series.bars.length>=PRICE_STRUCTURE_RULE.minimumBars&&card.atr!=null){
   const alternative=priceStructureAtr(series.bars.slice(14));
   card.seedDifference=alternative==null?null:Math.abs(alternative/card.atr-1);
   if(card.seedDifference==null||card.seedDifference>PRICE_STRUCTURE_RULE.seedTolerance){card.side='SIN DATOS';card.reason='ATR sensible a su inicio; esperando más historial.';}
  }
  return card;
 });
}

export interface StructureSummary {label:string;side:'CALL'|'PUT'|'MIXTA'|'PARCIAL';call:number;put:number;mixed:number;missing:number;complete:number;detail:string}
/** Exact descriptive alignment, never a majority trade signal or a calibrated probability. */
export function priceStructureSummary(cards:PriceStructureCard[]):StructureSummary {
 const canonical=PRICE_INTERVALS.map(interval=>cards.find(c=>c.interval===interval));
 const unique=cards.length===5&&new Set(cards.map(c=>c.interval)).size===5;
 const call=canonical.filter(c=>c?.side==='CALL').length,put=canonical.filter(c=>c?.side==='PUT').length,mixed=canonical.filter(c=>c?.side==='MIXTA').length;
 const missing=unique?5-call-put-mixed:5,complete=5-missing;
 const side=missing?'PARCIAL':call===5?'CALL':put===5?'PUT':'MIXTA';
 return {label:side==='PARCIAL'?'LECTURA PARCIAL':side==='MIXTA'?'DIVERGENCIA':'ALINEACIÓN '+side,side,call:unique?call:0,put:unique?put:0,mixed:unique?mixed:0,missing,complete,
 detail:missing?'Faltan '+missing+' temporalidades verificables; no se concluye alineación de las cinco.':side==='MIXTA'?'Las cinco escalas no comparten la misma estructura direccional.':'Las cinco temporalidades tienen estructura '+side+'. Alineación descriptiva de escalas solapadas; no probabilidad ni orden.'};
}
