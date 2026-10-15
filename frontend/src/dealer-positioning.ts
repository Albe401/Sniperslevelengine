import type {LiveState} from './live';
import {currentMapHorizon} from './reading-display';
import {chartContractReport} from './contract-profile';
import {interpretationFresh} from './confirmations';
import {escapeHtml as esc} from './model';
export interface DealerPositioning {version:string;available:boolean;at:number;expiration:string|null;headline:string;hhi:number|null;effectiveStrikes:number|null;rows:{strike:number;roles:string[];share:number;balance:number;gross:number;net:number;change:number|null;priceEffect:number|null;residual:number|null;oiChange:number|null;state:string}[]}
/** Arithmetic attribution, not a reconstruction of market-maker inventory. */
export function decomposeGex(oldGex:number,newGex:number,oldSpot:number,newSpot:number){
 if(![oldGex,newGex,oldSpot,newSpot].every(Number.isFinite)||oldGex<0||newGex<0||oldSpot<=0||newSpot<=0)return null;
 const change=newGex-oldGex,priceEffect=oldGex*((newSpot/oldSpot)**2-1),residual=change-priceEffect;
 return [change,priceEffect,residual].every(Number.isFinite)?{change,priceEffect,residual}:null;
}
export class DealerPositioningTracker {
 private baseline?:{scope:string;at:number;spot:number;rows:Map<number,{gross:number;oi:number}>};
 private cached?:DealerPositioning;
 update(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):DealerPositioning {
  const empty:DealerPositioning={version:'dealer-exposure-v1',available:false,at:now,expiration:null,headline:'Exposición por strike pendiente',hhi:null,effectiveStrikes:null,rows:[]};
  const h=currentMapHorizon(s?.optionSnapshotStatus,now),r=chartContractReport(s,symbol,active,h,now);
  const ds=r?.enrichment?.details;
  if(!s||!active||s.symbol!==symbol||!interpretationFresh(s.publishedMs,now)||h?.mode!=='DAY'||!h.structure.complete||!r?.gammaComplete||!r.enrichment?.priceFresh||!ds?.length||!Number.isFinite(r.spot)||r.spot<=0||ds.some(c=>[c.oi,c.gamma,c.gex].some(v=>v==null||!Number.isFinite(v)||v<0))) {this.baseline=undefined;this.cached=undefined;return empty;}
  const scope=JSON.stringify([symbol,s.startedMs,h.expirations,ds.map(c=>c.ticker).sort(),r.enrichment?.oiReferenceDate]);
  const old=this.baseline;
  if(old?.scope===scope&&old.at===r.at&&this.cached)return this.cached;
  const comparable=old?.scope===scope&&r.at>old.at&&r.at-old.at<=900000;
  const total=r.rows.reduce((n,x)=>n+(x.call.gamma??0)+(x.put.gamma??0),0);
  if(!Number.isFinite(total)||total<=0){this.baseline=undefined;this.cached=undefined;return empty;}
  const refs:[string,number|null|undefined][]=[['PUT WALL',h.structure.putWall?.strike],['CALL WALL',h.structure.callWall?.strike],['IMÁN PRINCIPAL',h.structure.selected.M1],['IMÁN DE LIQUIDEZ',h.structure.magnet]];
  const rows=r.rows.map(x=>{
   const gross=x.call.gamma!+x.put.gamma!,net=x.call.gamma!-x.put.gamma!,oi=x.call.oi!+x.put.oi!,prev=comparable?old!.rows.get(x.strike):undefined;
   const d=prev?decomposeGex(prev.gross,gross,old!.spot,r.spot):null;
   // A label is emitted only after a material, comparable residual (5% heuristic).
   const state=!d?'Primera referencia':prev!.gross===0?'Nueva exposición en el perfil':Math.abs(d.residual)<prev!.gross*.05?'Sin cambio material fuera del precio':d.residual>0?'Exposición aumenta fuera del efecto precio':'Exposición disminuye fuera del efecto precio';
   return {strike:x.strike,roles:refs.filter(([,strike])=>strike===x.strike).map(([name])=>name),share:100*gross/total,balance:gross>0?net/gross:0,gross,net,change:d?.change??null,priceEffect:d?.priceEffect??null,residual:d?.residual??null,oiChange:prev?oi-prev.oi:null,state};
  }).sort((a,b)=>b.share-a.share||a.strike-b.strike);
  const hhi=rows.reduce((n,x)=>n+(x.share/100)**2,0);
  const value:DealerPositioning={version:'dealer-exposure-v1',available:true,at:r.at,expiration:h.expirations[0],headline:rows.length?`Mayor concentración gamma: ${rows[0].strike.toFixed(2)}`:empty.headline,hhi,effectiveStrikes:hhi>0?1/hhi:null,rows};
  this.baseline={scope,at:r.at,spot:r.spot,rows:new Map(rows.map(x=>[x.strike,{gross:x.gross,oi:r.rows.find(y=>y.strike===x.strike)!.call.oi!+r.rows.find(y=>y.strike===x.strike)!.put.oi!}]))};this.cached=value;return value;
 }
}
const trackers=new Map<string,DealerPositioningTracker>();
export function readDealerPositioning(s:LiveState|undefined,symbol:string,active:boolean,now:number){let tracker=trackers.get(symbol);if(!tracker){tracker=new DealerPositioningTracker();if(trackers.size>=8)trackers.delete(trackers.keys().next().value!);trackers.set(symbol,tracker);}return tracker.update(s,symbol,active,now);}
export function dealerPositioningPanel(p:DealerPositioning|undefined){
 if(!p?.available)return '<p class="dealer-momentum">Exposición por strike: esperando cadena DAY completa y vigente.</p>';
 const visible=p.rows.filter(x=>x.roles.length).slice(0,4);
 return `<details class="dealer-combined-evidence"><summary>EXPOSICIÓN POR STRIKE · ${esc(p.headline)}</summary><p>Vence ${esc(p.expiration??'')} · comparación entre snapshots del mismo universo. Inventario real de market makers: no identificado.</p>${visible.map(x=>`<p><b>${esc(x.roles.join(' / '))} · ${x.strike.toFixed(2)}</b> · ${x.share.toFixed(1)}% del GEX bruto · ${esc(x.state)}<br>Balance +CALL/−PUT ${(x.balance*100).toFixed(1)}%. ${x.oiChange==null?'Sin comparación de OI':x.oiChange===0?'OI sin cambio observado':'Cambio de OI observado: '+x.oiChange.toFixed(0)+' contratos; no identifica aperturas/cierres intradía'}.</p>`).join('')}<p>GEX = gamma × OI × multiplicador × precio² × 0,01, en USD por movimiento del 1%. Efecto precio = GEX anterior × [(precio nuevo/precio anterior)² − 1]. Residuo = cambio total − efecto precio; incluye gamma, IV, tiempo y OI, no compras identificadas. Umbral descriptivo del residuo: 5% del GEX anterior, pendiente de calibración. HHI ${p.hhi?.toFixed(3)} · strikes efectivos ${p.effectiveStrikes?.toFixed(1)}. Concentración no equivale a liquidez pasiva ni probabilidad. Fuente conceptual: <a href="https://cdn.cboe.com/resources/education/research_publications/gammasqueezes.pdf" target="_blank" rel="noopener noreferrer">investigación Cboe sobre cobertura gamma</a>.</p></details>`;
}

