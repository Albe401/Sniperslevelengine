import {invoke,isTauri} from './snapshot-observation';
import type {EtfReport} from './etf-dealers';
import {escapeHtml as esc} from './model';
import {topFiveAmplitude} from './top-five-amplitude';
import './scanner-dealers.css';
export interface NativeCandidate {symbol:string;volume:number;price:number;changePct:number;volumeMetrics?:{rvolPremarket:number|null}}
export let nativeHorizon=7;
export const nativeReports:Record<string,EtfReport>={};
const errors:Record<string,string>={};
const requests=new Map<string,number>();
let busy=false,generation=0,revision=0;
export const nativeRevision=()=>revision;
let deferred:{rows:NativeCandidate[];paint:()=>void;force:boolean}|undefined;
const day=(at:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(at);
const valid=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>=0;
const fmt=(n:number|null)=>n==null?'—':new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
export function usableNative(r:EtfReport|undefined,symbol:string,now=Date.now(),requirePrice=true):r is EtfReport {
 return !!r&&r.symbol===symbol&&r.days===nativeHorizon&&r.at<=now&&now-r.at<=300000&&day(r.at)===day(now)&&Number.isFinite(r.spot)&&r.spot>0&&r.rows.length>0&&new Set(r.rows.map(x=>x.strike)).size===r.rows.length&&r.rows.every(x=>Number.isFinite(x.strike)&&x.strike>0)&&!!r.enrichment&&(!requirePrice||r.enrichment.priceFresh);
}
export function nativeMetrics(r:EtfReport){
 const sum=(metric:'oi'|'volume'|'delta'|'gamma',side?:'call'|'put')=>{
  const values=r.rows.flatMap(x=>side?[x[side][metric]]:[x.call[metric],x.put[metric]]);
  return values.length&&values.every(valid)?values.reduce<number>((a,b)=>a+(b as number),0):null;
 };
 const oi=sum('oi'),volume=sum('volume'),gex=sum('gamma'),dex=sum('delta');
 const details=r.enrichment?.details??[];
 const known=details.filter(x=>valid(x.gex)&&valid(x.oi));
 const knownOi=known.reduce((a,b)=>a+b.oi!,0);
 const coverage=r.gammaComplete?1:oi&&details.length?knownOi/oi:0;
 const observedGex=gex??(known.length?known.reduce((a,b)=>a+b.gex!,0):null);
 const byStrike=new Map<number,number>();known.forEach(x=>byStrike.set(x.strike,(byStrike.get(x.strike)??0)+x.gex!));
 const ranked=(gex!=null?r.rows.filter(x=>valid(x.call.gamma)&&valid(x.put.gamma)).map(x=>({strike:x.strike,gex:x.call.gamma!+x.put.gamma!})):Array.from(byStrike,([strike,gex])=>({strike,gex}))).map(x=>({...x,distance:100*Math.abs(x.strike/r.spot-1)})).sort((a,b)=>b.gex-a.gex||a.strike-b.strike);
 const near=ranked.filter(x=>x.distance<=2).reduce((a,b)=>a+b.gex,0);
 return {oi,volume,gex,observedGex,coverage,dex,callOi:sum('oi','call'),putOi:sum('oi','put'),callVolume:sum('volume','call'),putVolume:sum('volume','put'),volumeOi:oi&&volume!=null?volume/oi:null,nearShare:observedGex?near/observedGex:null,ranked:ranked.slice(0,3)};
}
/** Descriptive opportunity priority: observed activity plus nearby exposure, never a probability or dealer sign. */
export function nativePriority(c:NativeCandidate,r:EtfReport|undefined,now=Date.now()):number|null{
 if(!usableNative(r,c.symbol,now))return null;
 const m=nativeMetrics(r);if(m.oi==null||m.oi<=0||m.volumeOi==null||m.nearShare==null||!m.ranked.length||m.coverage<.9||m.coverage>1)return null;
 const rvol=c.volumeMetrics?.rvolPremarket;
 return 100*m.coverage*(.4*Math.min(1,m.volumeOi)+.25*m.nearShare+.2*(valid(rvol)?Math.min(1,rvol/3):0)+.15*Math.min(1,Math.abs(c.changePct)/3));
}
export function shortlistNative<T extends NativeCandidate>(rows:T[]):T[]{
 const good=rows.filter(c=>c.symbol&&c.volume>0&&Number.isFinite(c.volume)&&c.price>0&&Number.isFinite(c.price)&&Number.isFinite(c.changePct));
 return good.filter(c=>good.filter(x=>x.symbol===c.symbol).length===1).slice().sort((a,b)=>b.volume-a.volume||a.symbol.localeCompare(b.symbol)).slice(0,20);
}
export function rankNative<T extends NativeCandidate>(rows:T[],now=Date.now(),reports=nativeReports):T[]{
 const pool=shortlistNative(rows),scores=new Map(pool.map(c=>[c.symbol,nativePriority(c,reports[c.symbol],now)??-1]));
 return pool.sort((a,b)=>scores.get(b.symbol)!-scores.get(a.symbol)!||b.volume-a.volume||a.symbol.localeCompare(b.symbol)).slice(0,5);
}
export function nativeOptionsPanel(c:NativeCandidate,now=Date.now(),enabled=true):string{
 const r=enabled?nativeReports[c.symbol]:undefined,ready=usableNative(r,c.symbol,now),info=usableNative(r,c.symbol,now,false),m=info?nativeMetrics(r):undefined,score=nativePriority(c,r,now);
 const empty=!!r&&r.symbol===c.symbol&&r.days===nativeHorizon&&r.at<=now&&now-r.at<=300000&&r.rows.length===0;
 const reason=empty?(nativeHorizon===0?'Sin contratos 0DTE para este activo':'Sin contratos en este horizonte'):info&&!ready?'Precio de referencia antiguo: no clasifica':r&&r.at<=now&&now-r.at>300000?'Snapshot caducado; pendiente de actualización':errors[c.symbol]||errors.consulta||(busy?'Consultando opciones…':'Pendiente de consulta nativa');
 return `<div class="scanner-dealers native-options"><h3>OPCIONES · SNIPERS NATIVO</h3><p>Prioridad ${score==null?'—':score.toFixed(0)+'/100'+(r?.gammaComplete?'':' · provisional')} · ${nativeHorizon===0?'0DTE':nativeHorizon+' días'} · ${ready?'snapshot vigente':info?'snapshot informativo':'sin cadena vigente'}</p><div class="native-options-scroll"><p>OI CALL / PUT <b>${fmt(m?.callOi??null)} / ${fmt(m?.putOi??null)}</b></p><p>Volumen CALL / PUT <b>${fmt(m?.callVolume??null)} / ${fmt(m?.putVolume??null)}</b></p><p>GEX absoluto <b>${fmt(m?.observedGex??null)} USD/1% ${m&&m.gex==null?'· parcial':''}</b></p><p>DEX absoluto <b>${fmt(m?.dex??null)} acciones</b></p><p>Volumen/OI <b>${m?.volumeOi==null?'—':m.volumeOi.toFixed(2)+'×'}</b></p><p>GEX observado a ±2% del precio <b>${m?.nearShare==null?'—':(m.nearShare*100).toFixed(0)+'%'}</b></p><h3>STRIKES CON MAYOR GEX</h3>${m?.ranked.map(x=>`<p>$${x.strike.toFixed(2)} · ${fmt(x.gex)} · a ${x.distance.toFixed(2)}%</p>`).join('')||'<p>Sin concentración verificable</p>'}<small>${info?`${r.contracts} series · ${r.expirations.join(' · ')} · gamma ${r.gammaComplete?'completa':'parcial'} · cobertura por OI ${((m?.coverage??0)*100).toFixed(0)}% · OI conocido ${r.enrichment!.oiKnown}/${r.enrichment!.details.length} · ${ready?'precio reciente':'precio antiguo: no clasifica'}`:esc(reason)}</small>${topFiveAmplitude(info?r:undefined,now)}</div></div>`;
}
export function setNativeHorizon(days:number){if(![0,7,28].includes(days)||days===nativeHorizon)return;nativeHorizon=days;generation++;revision++;requests.clear();for(const s of Object.keys(nativeReports))delete nativeReports[s];}
export async function refreshNativeOptions(rows:NativeCandidate[],paint:()=>void,force=false){
 if(!isTauri()||!document.querySelector('[data-manual-scan-root]'))return;
 if(busy){deferred={rows,paint,force};return;}
 const symbols=shortlistNative(rows).map(c=>c.symbol).filter(s=>force||Date.now()-(requests.get(s)??0)>=60000);if(!symbols.length)return;
 busy=true;const current=generation,days=nativeHorizon;symbols.forEach(s=>requests.set(s,Date.now()));
 try{const result=await invoke<{reports:Record<string,EtfReport>;errors:Record<string,string>}>('get_manual_scanner_profiles',{symbols,maxDte:days});if(current===generation){for(const s of symbols){delete nativeReports[s];delete errors[s];}Object.assign(nativeReports,result.reports);Object.assign(errors,result.errors);}}
 catch(e){if(current===generation)symbols.forEach(s=>{delete nativeReports[s];errors[s]=String(e);});}
 finally{if(current===generation)symbols.forEach(s=>requests.set(s,Date.now()));busy=false;revision++;paint();const next=deferred;deferred=undefined;if(next)void refreshNativeOptions(next.rows,next.paint,next.force);}
}
