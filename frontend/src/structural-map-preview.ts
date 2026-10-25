import {priceStructureMap} from './price-structure-map';
import {structuralQuality,type StructuralQuality} from './structural-quality';
import {qqqConfirmation} from './qqq-confirmation';
import {atrMeter} from './atr-meter';
import {structuralAtr} from './atr-context';
import type {AtrRead} from './intraday-atr';
import {CalendarDays,Clock3,type IconNode} from 'lucide';
import {escapeHtml as esc} from './model';
import type {LiveState} from './live';
import {rangeFresh} from './daily-range';
import {cvdConfluence,type CvdConfluence} from './cvd-confluence';
import './structural-map-preview.css';
import {structuralEnrichmentPanel} from './structural-enrichment';
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const amount=(n:number|null)=>n==null?'—':(n>0?'+':'')+new Intl.NumberFormat('en-US',{notation:'compact',maximumFractionDigits:2}).format(n);
export interface StructuralCard {quality?:StructuralQuality;partial?:boolean;coverage?:number;components?:import('./qqq-confirmation').QqqConfirmation;scopeReady?:boolean;expirations?:string[];vanna?:import('./vanna-engine').VannaPair;name:string;side:'CALL'|'PUT'|'NEUTRAL'|'PENDIENTE';call:number|null;put:number|null;callDelta:number|null;putDelta:number|null;net:number|null;detail:string;pendingReason?:string;cvd:CvdConfluence;at?:number;atr?:{label:string;detail:string}}
const etFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
function validDate(d:string):boolean {return /^\d{4}-\d{2}-\d{2}$/.test(d)&&!Number.isNaN(Date.parse(d+'T12:00:00Z'))&&new Date(d+'T12:00:00Z').toISOString().slice(0,10)===d;}
function scope(mode:string,dates:string[],today:string,universe:string[]):boolean {
 if(!Array.isArray(dates)||!Array.isArray(universe)||!dates.length||dates.some(d=>!validDate(d)||d<today)||new Set(dates).size!==dates.length||dates.some((d,i)=>i>0&&d<=dates[i-1]))return false;
 const day=new Date(today+'T12:00:00Z'),offset=(5-day.getUTCDay()+7)%7;day.setUTCDate(day.getUTCDate()+offset);const friday=day.toISOString().slice(0,10);
 const all=[...new Set(universe.filter(d=>validDate(d)&&d>=today))].sort();
 const expected=mode==='DAY'?[today]:mode==='NEXT_SESSION'?all.filter(d=>d>today).slice(0,1):mode==='WEEK'?all.filter(d=>d<=friday):all.slice(0,4);
 const shape=mode==='DAY'?dates.length===1&&dates[0]===today:mode==='NEXT_SESSION'?dates.length===1&&dates[0]>today:mode==='WEEK'?dates.every(d=>d<=friday):dates.length===4;
 return shape&&(!universe.length||dates.join('|')===expected.join('|'));
}
export function structuralAlignment(cards:StructuralCard[]):{label:string;detail:string} {
 if(cards.length!==3||cards.some(c=>c.side==='PENDIENTE'||c.partial)||cards.some(c=>c.at!==cards[0].at))return {label:'ALINEACIÓN DELTA · PENDIENTE',detail:'Requiere tres horizontes válidos del mismo snapshot, con cuatro expiraciones distintas en NEXT4.'};
 const side=cards.every(c=>c.side==='CALL')?'CALL':cards.every(c=>c.side==='PUT')?'PUT':null;
 return {label:side?`DELTA ${side} · 3/3`:cards.every(c=>c.side==='NEUTRAL')?'DELTA NEUTRAL · 3/3':'DELTA MIXTO',detail:'Alineación de exposición delta ponderada por OI dentro del rango de strikes consultado. Los horizontes se solapan; no son tres votos independientes, ni garantizan dirección futura o una entrada.'};
}
/** Exposure is OI-weighted delta in equivalent shares, not traded premium or a success probability. */
export function structuralCards(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),atr?:AtrRead,vanna?:import('./vanna-engine').VannaEngine):StructuralCard[] {
 const snapshot=s?.optionSnapshotStatus;
 const usable=active&&s?.symbol===symbol&&snapshot?.phase==='ready'&&rangeFresh(snapshot,now);
 const today=etFormatter.format(now);
 return ['DAY','WEEK','NEXT4'].map((mode,i)=>{
  const candidates=usable&&Array.isArray(snapshot.horizons)?snapshot.horizons.filter(x=>x.mode===mode||(mode==='DAY'&&x.mode==='NEXT_SESSION')):[];
  const h=candidates.length===1?candidates[0]:undefined;
  const e=h?.exposure;
  const gross=e&&finite(e.callDex)&&finite(e.putDex)?Math.abs(e.callDex)+Math.abs(e.putDex):0;
  const scopeReady=!!h&&scope(h.mode,h.expirations,today,snapshot?.expirations??[]);
  const ok=scopeReady&&!!h&&h.observedMs===snapshot?.observedMs&&finite(h.observedMs)&&etFormatter.format(h.observedMs)===today&&rangeFresh({phase:'ready',observedMs:h.observedMs},now)&&e?.dexComplete===true&&Number.isInteger(e.eligible)&&e.eligible>0&&Number.isInteger(e.deltaUsed)&&e.deltaUsed===e.eligible&&e.unknownOi===0&&gross>0&&finite(e.callDex)&&finite(e.putDex)&&finite(e.netDex)&&finite(e.callDexPct)&&finite(e.putDexPct)&&e.callDex>=0&&e.putDex<=0&&Math.abs(e.netDex-e.callDex-e.putDex)<Math.max(1e-6,gross*1e-9)&&e.callDexPct>=0&&e.callDexPct<=100&&e.putDexPct>=0&&e.putDexPct<=100&&Math.abs(e.callDexPct+e.putDexPct-100)<.01&&Math.abs(e.callDexPct-100*Math.abs(e.callDex)/gross)<.01;
  const partial=scopeReady&&!!h&&h.observedMs===snapshot?.observedMs&&etFormatter.format(h.observedMs)===today&&rangeFresh({phase:'ready',observedMs:h.observedMs},now)&&e?.dexComplete===false&&Number.isInteger(e.eligible)&&e.eligible>0&&Number.isInteger(e.deltaUsed)&&e.deltaUsed>0&&e.deltaUsed<=e.eligible&&(e.deltaUsed<e.eligible||e.unknownOi>0)&&Number.isInteger(e.unknownOi)&&e.unknownOi>=0&&e.unknownOi<=e.eligible&&gross>0&&finite(e.callDex)&&finite(e.putDex)&&finite(e.netDex)&&e.callDex>=0&&e.putDex<=0&&Math.abs(e.netDex-e.callDex-e.putDex)<Math.max(1e-6,gross*1e-9);
  const pendingReason=!active?'Conexión de mercado pendiente':s?.symbol!==symbol?'Activo seleccionado sin snapshot':snapshot?.phase!=='ready'?'Snapshot de opciones '+(snapshot?.phase??'pendiente'):!rangeFresh(snapshot,now)?'Snapshot de opciones vencido':!h?'Horizonte de opciones no recibido':!scopeReady?(mode==='NEXT4'&&h.expirations.length<4?'Faltan expiraciones para los próximos 4 vencimientos':'Vencimientos del horizonte no verificables'):h.observedMs!==snapshot.observedMs?'Horizontes de snapshots distintos':partial?'Delta parcial: '+e!.deltaUsed+'/'+e!.eligible+' contratos':!ok?'Delta u OI incompleto':'';
  const call=ok?e!.callDexPct!:partial?100*Math.abs(e!.callDex!)/gross:null,put=call==null?null:100-call;
  const side:StructuralCard['side']=!ok||call==null?'PENDIENTE':call>=52.5?'CALL':call<=47.5?'PUT':'NEUTRAL';
  const cvd=cvdConfluence(s,{call,put,side,state:'Exposición delta',reason:'',components:[]},symbol,active,now);
  const dates=h?.expirations??[]; const tick=s?.ticks?.[symbol]; const spot=active&&s?.symbol===symbol&&s.stocks?.phase==='streaming'&&s.publishedMs<=now&&now-s.publishedMs<=3000&&tick?.symbol===symbol&&finite(tick.price)&&tick.price>0&&finite(tick.tradeMs)&&tick.tradeMs<=now&&now-tick.tradeMs<=15000?tick.price:null;
  return {quality:structuralQuality(call,!!partial,e?.callShareBounds,e?.deltaCapacityCoveragePct),partial,coverage:e&&e.eligible>0?100*e.deltaUsed/e.eligible:undefined,scopeReady,expirations:dates,vanna:vanna?.symbol===symbol&&vanna.at===now?vanna.structure[i]?.support:undefined,at:h?.observedMs,atr:atr?structuralAtr(atr,h,side,spot):undefined,name:i===0?(h?.mode==='NEXT_SESSION'?'PRÓX. VENC.':'DIARIO'):i===1?'SEMANAL':'PRÓX. 4 EXP.',side,call,put,callDelta:ok||partial?e!.callDex!:null,putDelta:ok||partial?e!.putDex!:null,net:ok||partial?e!.netDex:null,pendingReason:side==='PENDIENTE'?pendingReason:undefined,
   detail:(h?.mode==='NEXT_SESSION'?'Próxima sesión · ':'')+(dates.length?dates.join(' / '):'Vencimientos pendientes')+(ok?' · delta completo':partial?' · delta parcial: '+e!.deltaUsed+'/'+e!.eligible+' contratos':' · exposición pendiente o incompleta')+(mode==='NEXT4'&&dates.length<4?' · menos de 4 expiraciones disponibles':''),
   components:qqqConfirmation(s,symbol,active,side,now),cvd:{...cvd,detail:cvd.detail+' CVD de acciones del tramo actual: contexto del horizonte, no CVD acumulado semanal ni por expiración.'}};
 });
}
const icon=(nodes:IconNode)=>`<svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">${(nodes[2]??[]).map(([tag,attrs])=>`<${tag} ${Object.entries(attrs).filter(([k])=>k!=='key').map(([k,v])=>`${k}="${esc(String(v))}"`).join(' ')}/>`).join('')}</svg>`;
export function structuralMapPreview(symbol:string,s?:LiveState,active=false,now=Date.now(),atr?:AtrRead,vanna?:import('./vanna-engine').VannaEngine):string {
 return priceStructureMap(symbol,s,active,now,structuralCards(s,symbol,active,now,atr,vanna));
}
