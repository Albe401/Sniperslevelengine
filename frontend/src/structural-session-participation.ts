import type {StructuralMapContext,ActivityContext} from './structural-map-context';
import type {FiveMinuteStructureCard} from './price-structure';
import type {SessionStructurePair} from './session-structure-pair';
import {validStructuralIndicators} from './structural-indicators';
import {escapeHtml as esc} from './model';
export interface SessionParticipation {
 version:'structural-session-participation-v1';symbol:string;publishedMs:number|null;atMs:number;availability:'current'|'reference'|'pending';
 side:'CALL'|'PUT'|null;
 cvd:{state:'acompaña'|'contradice'|'neutral'|'pendiente';signal:number|null;coverage:number|null;atMs:number|null;reason:string};
 flow:{state:'contexto parcial'|'pendiente';signal:number|null;coverage:number|null;atMs:number|null;reason:string};
 rvol:{value:number|null;references:number;required:20;atMs:number|null;reason:string};
 gex:{state:'contexto'|'parcial'|'pendiente';net:number|null;unit:'USD-per-1pct';atMs:number|null;coverage:number|null;reason:string};
 probability:null;entryAuthorized:false;
}
/** Reuse source-quality contracts. Activity, premiums and OI are distinct measurements, never votes. */
export function readSessionParticipation(c:StructuralMapContext,five:FiveMinuteStructureCard,pair:SessionStructurePair):SessionParticipation {
 const r:SessionParticipation={version:'structural-session-participation-v1',symbol:c.symbol,publishedMs:c.framePublishedMs,atMs:c.atMs,availability:'pending',side:null,cvd:{state:'pendiente',signal:null,coverage:null,atMs:null,reason:'Sin ventana CVD verificable de cinco minutos.'},flow:{state:'pendiente',signal:null,coverage:null,atMs:null,reason:'Sin prima comparable; no se presume recepción continua.'},rvol:{value:null,references:0,required:20,atMs:null,reason:'Sin veinte sesiones comparables a la misma hora.'},gex:{state:'pendiente',net:null,unit:'USD-per-1pct',atMs:null,coverage:null,reason:'Sin exposición Massive asociada.'},probability:null,entryAuthorized:false};
 const time=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
 if(c.frame.status==='rejected'||!time(c.atMs)||!time(c.framePublishedMs)||c.framePublishedMs>c.atMs||c.atMs-c.framePublishedMs>15000||five.sourceSymbol!==c.symbol||five.sourcePublishedMs!==c.framePublishedMs||pair.symbol!==c.symbol||pair.five!==five)return r;
 r.availability=c.frame.freshness==='current'&&pair.availability==='current'?'current':'reference';
 r.side=pair.availability!=='pending'&&(pair.side==='CALL'||pair.side==='PUT')?pair.side:null;
 const valid=(v:ActivityContext)=>v.freshness==='current'&&v.intervalMinutes===5&&time(v.observedMs)&&v.observedMs<=c.framePublishedMs!&&c.atMs-v.observedMs<=3000&&v.signal!=null&&Number.isFinite(v.signal)&&Math.abs(v.signal)<=1&&v.classifiedCoveragePct!=null&&Number.isFinite(v.classifiedCoveragePct)&&v.classifiedCoveragePct>=70&&v.classifiedCoveragePct<=100;
 const cv=c.activity.cvd;
 if(r.availability==='current'&&cv.status==='available'&&cv.temporalCoverage==='complete'&&valid(cv))r.cvd={state:r.side==null?'pendiente':cv.signal===0?'neutral':Math.sign(cv.signal!)===(r.side==='CALL'?1:-1)?'acompaña':'contradice',signal:cv.signal,coverage:cv.classifiedCoveragePct,atMs:cv.observedMs,reason:cv.reason};else r.cvd.reason=cv.reason;
 const f=c.activity.flow;
 // Classified premium coverage does not prove continuous reception. Preserve its partial status.
 if(r.availability==='current'&&f.status==='partial'&&valid(f))r.flow={state:'contexto parcial',signal:f.signal,coverage:f.classifiedCoveragePct,atMs:f.observedMs,reason:f.reason};else r.flow.reason=f.reason;
 const i=five.indicators;
 if(validStructuralIndicators(i,five.endMs,five.bars)&&i){r.rvol={value:i.rvol.value,references:i.rvol.references,required:20,atMs:five.endMs,reason:i.rvol.method+'; mide actividad, no dirección.'};}
 const g=c.options;
 if(g.source==='Massive'&&(g.status==='available'||g.status==='partial')&&g.id&&time(g.observedMs)&&g.observedMs<=c.framePublishedMs&&g.net!=null&&Number.isFinite(g.net))r.gex={state:g.status==='partial'?'parcial':'contexto',net:g.net,unit:g.unit,atMs:g.observedMs,coverage:g.countCoveragePct,reason:g.reason+' El signo de GEX no determina CALL ni PUT; cobertura de contratos no es cobertura de riesgo.'};else r.gex.reason=g.reason;
 return r;
}
export function sessionParticipationDetails(r:SessionParticipation):string {
 const x=(n:number|null)=>n==null?'pendiente':n.toFixed(4),t=(n:number|null)=>n==null?'sin captura':new Date(n).toLocaleString('es',{timeZone:'America/New_York',hour12:false})+' ET';
 return `<details data-horizon-details="structural-session-participation" data-session-participation="${r.availability}"><summary>Participación · CVD, volumen, opciones y GEX</summary><p>CVD de acciones 5m: ${esc(r.cvd.state)}. Balance normalizado ${x(r.cvd.signal)}; cobertura clasificada ${x(r.cvd.coverage)}%; ${esc(t(r.cvd.atMs))}. ${esc(r.cvd.reason)}</p><p>Prima de opciones 5m: ${esc(r.flow.state)}. Balance ${x(r.flow.signal)}; cobertura clasificada ${x(r.flow.coverage)}%; ${esc(t(r.flow.atMs))}. ${esc(r.flow.reason)}</p><p>RVOL 5m: ${x(r.rvol.value)} veces; ${r.rvol.references}/20 referencias. ${esc(r.rvol.reason)}</p><p>GEX Massive: ${esc(r.gex.state)}; neto ${x(r.gex.net)} USD por movimiento del1%; ${esc(t(r.gex.atMs))}. ${esc(r.gex.reason)}</p><p>${r.availability==='reference'?'Referencia histórica. ':''}No se suman acciones, primas y exposición ni se cuenta el mismo dato como varios votos. Ningún apoyo aislado confirma una entrada.</p></details>`;
}
