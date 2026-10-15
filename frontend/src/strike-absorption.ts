import {atrDistance} from './intraday-atr';
import {dailyDistance} from './atr-context';
import type {LiveState} from './live';
import {rangeFresh} from './daily-range';
import {interpretationFresh} from './confirmations';
import {displayStructure,mapHorizon} from './reading-display';
import {escapeHtml as esc} from './model';
import type {ControlReading} from './intraday-control';
export interface StrikeDelta {ticker:string;strike:number;kind:string;expiration:string;delta:number;gamma:number|null;oi:number|null;observedMs:number}
export interface StrikeReading {rows:{vanna?:import('./vanna-engine').VannaPair;forceReason?:string;name:string;strike:number;expiration:string;call:number|null;put:number|null;at:number|null;change:number|null;strength:number|null;pressure:string;partial:boolean}[]}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const dateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const et=(at:number)=>dateFormatter.format(at);
export function readStrikeAbsorption(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):StrikeReading {
 const empty={rows:[]};
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now)||s.optionSnapshotStatus?.phase!=='ready'||!rangeFresh(s.optionSnapshotStatus,now))return empty;
 const h=mapHorizon(s.optionSnapshotStatus.horizons),structure=displayStructure(h,!!h&&rangeFresh({phase:'ready',observedMs:h.observedMs},now));
 if(!structure||!h||h.observedMs!==s.optionSnapshotStatus.observedMs)return empty;
 const expiry=h.expirations.slice().sort()[0];if(!expiry)return empty;
 const levels:[string,number|null|undefined][]=[['CALL WALL',structure.callWall?.strike],['IMÁN PRINCIPAL',structure.selected.M1],['PUT WALL',structure.putWall?.strike],...structure.routes.map(r=>[`BARRERA ${r.direction}`,r.barrier] as [string,number|null])];
 const dynamics=expiry===et(now)&&s.levelDynamics?.symbol===symbol&&s.levelDynamics.available&&s.levelDynamics.at<=now&&now-s.levelDynamics.at<=3000?s.levelDynamics:undefined;
 const today=et(now);
 const valid=(s.levelDeltas??[]).filter(c=>finite(c.delta)&&Math.abs(c.delta)<=1&&c.observedMs===h.observedMs&&c.observedMs>0&&c.observedMs<=now&&now-c.observedMs<=900000&&et(c.observedMs)===today&&c.expiration===expiry&&((c.kind==='call'&&c.delta>=0)||(c.kind==='put'&&c.delta<=0)));
 return {rows:levels.filter((l):l is [string,number]=>finite(l[1])&&l[1]>0).map(([name,strike])=>{
  const contracts=valid.filter(c=>c.strike===strike);
  const one=(kind:string)=>{const rows=contracts.filter(c=>c.kind===kind);return rows.length===1?rows[0]:undefined;};
  const call=one('call'),put=one('put'),d=dynamics?.levels.find(l=>l.name===name&&l.strike===strike);
  const pressure=d?.flowComplete&&finite(d.classifiedPct)&&d.classifiedPct>=70&&d.classifiedPct<=100&&[d.callBuy,d.callSell,d.putBuy,d.putSell].every(v=>finite(v)&&v>=0)?(()=>{const buy=d.callBuy!+d.putSell!,sell=d.putBuy!+d.callSell!;return buy+sell<=0?'Sin flujo clasificado':buy>sell*1.2?'Opciones: presión alcista':sell>buy*1.2?'Opciones: presión bajista':'Opciones: sin predominio';})():'Presión pendiente / parcial';
  const forceReason=!dynamics?(expiry!==today?'Referencia de próximo vencimiento; fuerza intradía solo DAY':s.levelDynamics?.reason??'Sin dinámica vigente del activo'):!d?'Zona no disponible en el cálculo de fuerza':d.strength==null?`Cobertura ${d.coverage}/100; mínimo 60. Faltan componentes verificables de gamma, OI, volumen o prima reciente.`:'Fuerza del modelo; no probabilidad de rebote';
  return {forceReason,name,strike,expiration:expiry,call:call?.delta??null,put:put?.delta??null,at:call||put?Math.min(...[call,put].flatMap(c=>c?[c.observedMs]:[])):null,change:!d?.provisional&&finite(d?.change)?d.change:null,strength:finite(d?.strength)&&d.strength>=0&&d.strength<=100?d.strength:null,pressure,partial:!!structure&&'coveredContracts' in structure};
 })};
}
const strikeDeltaText=(v:number|null)=>v==null?'—':(v>0?'+':'')+v.toFixed(2);
const strikeStrengthText=(l:StrikeReading['rows'][number])=>l.strength==null?'Fuerza pendiente':`Fuerza ${l.strength.toFixed(1)}/100`;
export function strikeAbsorptionRows(r:ControlReading):string {
 const rows=r.strikeReading?.rows??[];
 return `<div class="strike-absorption-grid strike-absorption-compact">${rows.map(l=>`<article><header><b>${esc(l.name)} · ${l.strike.toFixed(2)}</b></header><div class="strike-deltas"><span>Δ CALL <b>${strikeDeltaText(l.call)}</b></span><span>Δ PUT <b>${strikeDeltaText(l.put)}</b></span><span class="strike-strength" title="${esc(l.forceReason??'Fuerza del modelo; no probabilidad')}">${strikeStrengthText(l)}</span></div></article>`).join('')||'<p>Strikes y delta pendientes de una cadena vigente.</p>'}</div>`;
}
export function strikeAbsorptionDetails(r:ControlReading):string {
 return (r.strikeReading?.rows??[]).map(l=>{
  const spot=r.priceNow?.ready?r.priceNow.spot:null;
  const distance=spot!=null?[atrDistance(r.atr,spot,l.strike),dailyDistance(r.atr,spot,l.strike)].filter(Boolean).join(' · '):'';
  return `<p><b>${esc(l.name)} · ${l.strike.toFixed(2)}</b> · ${esc(l.expiration)}${l.partial?' · provisional':''}<br>${distance?esc(distance)+' · ':''}${strikeStrengthText(l)} · ${esc(l.forceReason??'')} · ${l.change==null?'Cambio pendiente':l.change>=5?'↑ Ganando fuerza':l.change<=-5?'↓ Perdiendo fuerza':'→ Estable'} · ${esc(l.pressure)}${l.at==null?'':` · Snapshot ${new Date(l.at).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET`}</p>`;
 }).join('');
}
