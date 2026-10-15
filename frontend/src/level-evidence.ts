import type {LiveState} from './live';
import type {EtfDetail} from './etf-enrichment';
export interface LevelEvidence {at:number;expirations:string[];contracts:number;oi:number|null;oiChange:number|null;volume:number|null;volumeOi:number|null;dex:number|null;gex:number|null;iv:number|null;charm:number|null;vanna:number|null;oiShare:number|null;gammaShare:number|null;quality:'complete'|'partial';quotesFresh:number;detail:string}
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const sum=(v:(number|null)[])=>v.length&&v.every(finite)?v.reduce<number>((a,b)=>a+b!,0):null;
const dateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const day=(n:number)=>dateFormatter.format(n);
/** One exact contract universe per horizon. No dealer signs or zero-filled unknowns. */
export function dailyEvidenceContracts(s:LiveState|undefined,symbol:string,active:boolean,now:number):EtfDetail[]|undefined {
 const snap=s?.optionSnapshotStatus,r=snap?.structuralEnrichment,h=snap?.horizons.find(h=>h.mode==='DAY');
 if(!active||s?.symbol!==symbol||!finite(now)||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||snap?.phase!=='ready'||!r?.enrichment||!h||r.symbol!==symbol||r.at!==snap.observedMs||r.at!==h.observedMs||r.at>now||now-r.at>300000||day(r.at)!==day(now)||h.expirations.length!==1||h.expirations[0]!==day(now)||!r.expirations.includes(day(now)))return;
 const today=day(now),ds=r.enrichment.details.filter(c=>c.expiration===today);
 if(!ds.length||new Set(ds.map(c=>c.ticker)).size!==ds.length||ds.some(c=>!finite(c.strike)||c.strike<=0||!['call','put'].includes(c.kind)||[c.oi,c.volume,c.dex,c.gex].some(v=>v!=null&&(!finite(v)||v<0))))return;
 return ds;
}
export function evidenceAtStrike(ds:EtfDetail[],strike:number,at:number):LevelEvidence|undefined {
 const rows=ds.filter(c=>c.strike===strike);if(!rows.length)return;
 return aggregateLevel(rows,at,sum(ds.map(c=>c.oi)),sum(ds.map(c=>c.gex)));
}
/** Aggregate the contract universe once, even when several map roles share a strike. */
export function evidenceByStrike(ds:EtfDetail[],at:number):Map<number,LevelEvidence>{
 const groups=new Map<number,EtfDetail[]>();
 for(const c of ds){const rows=groups.get(c.strike)??[];rows.push(c);groups.set(c.strike,rows);}
 const totalOi=sum(ds.map(c=>c.oi)),totalGex=sum(ds.map(c=>c.gex));
 return new Map([...groups].map(([strike,rows])=>[strike,aggregateLevel(rows,at,totalOi,totalGex)]));
}
function aggregateLevel(rows:EtfDetail[],at:number,totalOi:number|null,totalGex:number|null):LevelEvidence {
 const oi=sum(rows.map(c=>c.oi)),volume=sum(rows.map(c=>c.volume)),gex=sum(rows.map(c=>c.gex)),dex=sum(rows.map(c=>c.dex));
 const oiShare=oi!=null&&totalOi!=null&&totalOi>0?100*oi/totalOi:null,gammaShare=gex!=null&&totalGex!=null&&totalGex>0?100*gex/totalGex:null;
 const quality=oi!=null&&volume!=null&&gex!=null&&dex!=null?'complete':'partial';
 return {at,expirations:[...new Set(rows.map(c=>c.expiration))],contracts:rows.length,oi,oiChange:sum(rows.map(c=>c.oiChange)),volume,volumeOi:oi!=null&&oi>0&&volume!=null?volume/oi:null,dex,gex,iv:rows.every(c=>finite(c.iv))?rows.reduce((a,c)=>a+c.iv!,0)/rows.length:null,charm:rows.every(c=>finite(c.charm))?rows.reduce((a,c)=>a+c.charm!,0)/rows.length:null,vanna:rows.every(c=>finite(c.vanna))?rows.reduce((a,c)=>a+c.vanna!,0)/rows.length:null,oiShare,gammaShare,quality,quotesFresh:rows.filter(c=>c.fresh).length,
 detail:`${quality==='complete'?'Concentración completa':'Concentración parcial'} · ${rows.length} series del día · OI ${oi??'pendiente'} · GEX ${gex==null?'pendiente':gex.toFixed(0)} USD/1%. OI diario; exposición absoluta, sin identificación de inventario. Charm/vanna unitarios de modelo, sin agregarlos como posiciones.`};
}
