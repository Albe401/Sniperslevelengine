import type {LiveState}from'./live';import type{SessionStructurePair}from'./session-structure-pair';import type{SessionLocation}from'./structural-session-location';import type{SessionParticipation}from'./structural-session-participation';import type{SessionDecision}from'./structural-session-decision';import{escapeHtml as esc}from'./model';
export interface SessionHealth{version:'structural-session-health-v1';symbol:string;atMs:number;publishedMs:number|null;state:'ready'|'partial'|'reference'|'rejected';issues:string[];executionEnabled:false;probability:null}
/** Final source-association audit. Missing support stays partial; rejection cannot retain a current entry label. */
export function readSessionHealth(s:LiveState|undefined,symbol:string,active:boolean,now:number,pair:SessionStructurePair,l:SessionLocation,p:SessionParticipation,d:SessionDecision,origin:'live'|'snapshot'='live'):SessionHealth{
 const out:SessionHealth={version:'structural-session-health-v1',symbol,atMs:now,publishedMs:s?.publishedMs??null,state:'rejected',issues:[],executionEnabled:false,probability:null};
 const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
 if(!clock(now)||!s||s.symbol!==symbol||!clock(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>15000||!active){out.issues.push('Publicación inactiva, antigua o incompatible.');return out;}
 if([pair.symbol,l.symbol,p.symbol,d.symbol].some(v=>v!==symbol)||pair.five.sourceSymbol!==symbol||[pair.five.sourcePublishedMs,l.publishedMs,p.publishedMs,d.publishedMs].some(v=>v!==s.publishedMs)||l.observedMs!==now||p.atMs!==now||d.atMs!==now){out.issues.push('Los componentes no comparten activo, publicación y evaluación.');return out;}
 if([pair.five.endMs,pair.fifteenCloseMs,l.priceAtMs].some(v=>v!==null&&(!clock(v)||v>s.publishedMs))||l.priceAtMs!==null&&l.priceAtMs!==pair.five.endMs||l.vwap&&l.vwap.atMs!==l.priceAtMs){out.issues.push('Hay cierres futuros o referencias de precio no comparables.');return out;}
 if(origin==='snapshot'||s.metrics?.phase!=='Sesión RTH'||s.stocks?.phase!=='streaming'||[pair.availability,l.availability,p.availability,d.availability].includes('reference')){out.state='reference';out.issues.push('Referencia histórica o fuera de la sesión regular; no entrada vigente.');return out;}
 out.state='partial';
 if(pair.availability==='pending')out.issues.push('Falta estructura5m/15m verificable.');else if(pair.side==='MIXTA'||!pair.technicalAgreement)out.issues.push('Estructura y tendencia todavía no coinciden.');
 if(!l.opening)out.issues.push('Rango inicial incompleto.');if(!l.vwap)out.issues.push('VWAP comparable pendiente.');if(!l.previous)out.issues.push('Sesión anterior sin referencia completa.');
 if(p.cvd.signal===null)out.issues.push('CVD5m sin tramo verificable.');else if(p.cvd.state!=='acompaña')out.issues.push('CVD no acompaña la estructura.');
 if(p.rvol.value===null)out.issues.push('Volumen sin veinte referencias comparables.');
 if(!out.issues.length)out.state='ready';
 // Partial options/GEX remain context; they cannot supply missing price or CVD confirmation.
 return out;
}
export function guardSessionDecision(d:SessionDecision,h:SessionHealth):SessionDecision{
 if(h.state==='rejected')return{...d,availability:'pending',state:'PARCIAL',side:null,display:'PARCIAL',label:'Lectura pendiente de asociación',reason:h.issues.join(' '),route:null};
 if(h.state==='reference'&&d.availability==='current')return{...d,availability:'reference',state:d.state==='CONFIRMACIÓN'?'PREPARACIÓN':d.state,label:'Referencia de sesión',reason:h.issues.join(' '),route:null};
 return d;
}
export function sessionHealthDetails(h:SessionHealth):string{return`<details data-horizon-details="structural-session-health" data-session-health="${h.state}"><summary>Calidad de la lectura y datos pendientes</summary><p>${h.state==='ready'?'Componentes requeridos asociados y comparables. Esto no garantiza dirección ni autoriza una orden.':esc(h.issues.join(' '))}</p><p>Opciones y GEX son contexto: no sustituyen precio ni CVD ausentes. La dirección, los niveles y el gatillo conservan su motor original. Sin probabilidad calibrada ni ejecución automática.</p></details>`;}
