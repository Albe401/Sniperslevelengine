import {massiveExposurePanel} from './massive-exposure-panel';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import './quant-fall-reading.css';

export interface QuantFallReading {
 priceMovePct:number|null;
 volumeRatio:number|null;
 bearishFlowPct:number|null;
 classifiedPct:number|null;
 gex:number|null;
 gexPartial:number|null;
 gexCoverage:string|null;
 gexRegime:'POSITIVE'|'NEGATIVE'|'NEUTRAL'|'PENDING';
 gexHorizon:string|null;
 criteria:number|null;
 asOfMs:number|null;
}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const dateET=(ms:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(ms);
/** Closed, consecutive one-minute bars only. Previous ten bars form the local volume baseline. */
export function quantFallReading(s:LiveState|undefined,symbol:string,active:boolean,now:number):QuantFallReading {
 const out:QuantFallReading={priceMovePct:null,volumeRatio:null,bearishFlowPct:null,classifiedPct:null,gex:null,gexPartial:null,gexCoverage:null,gexRegime:'PENDING',gexHorizon:null,criteria:null,asOfMs:null};
 if(!active||s?.symbol!==symbol||!finite(now)||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>15000)return out;
 const session=s.chartHistory?.sessions?.find(x=>x.date===dateET(now));
 const bars=s.chartHistory?.minutes?.filter(b=>session&&b.timestampMs>=session.openMs&&b.timestampMs<session.closeMs&&b.timestampMs+60000<=now&&[b.timestampMs,b.open,b.high,b.low,b.close,b.volume].every(finite)&&b.close>0&&b.volume>=0).sort((a,b)=>a.timestampMs-b.timestampMs)??[];
 const recent=bars.slice(-13);
 if(recent.length===13&&recent.every((b,i)=>i===0||b.timestampMs===recent[i-1].timestampMs+60000)&&now-recent[12].timestampMs<=120000){
  const baseline=recent.slice(0,10).reduce((sum,b)=>sum+b.volume,0)/10;
  const current=recent.slice(10).reduce((sum,b)=>sum+b.volume,0)/3;
  if(baseline>0&&recent[9].close>0){out.priceMovePct=100*(recent[12].close/recent[9].close-1);out.volumeRatio=current/baseline;out.asOfMs=recent[12].timestampMs+60000;}
 }
 const flow=s.flow?.intraday_windows?.find(w=>w.minutes===5)??s.flow?.windows?.find(w=>w.minutes===5);
 const buckets=flow?.buckets;
 const classified=['CALL BUY','CALL SELL','PUT BUY','PUT SELL'].reduce((sum,k)=>sum+(finite(buckets?.[k]?.contracts)&&buckets![k].contracts>=0?buckets![k].contracts:0),0);
 const unknown=finite(buckets?.UNKNOWN?.contracts)&&buckets!.UNKNOWN.contracts>=0?buckets!.UNKNOWN.contracts:0;
 if(s.options?.phase==='streaming'&&finite(s.options.lastEventMs)&&s.options.lastEventMs<=now&&now-s.options.lastEventMs<=15000&&finite(s.flow?.asof_ms)&&s.flow.asof_ms<=now&&now-s.flow.asof_ms<=15000&&flow?.event_count&&flow.premium_complete&&classified+unknown>0&&finite(flow.bearish_pct)&&flow.bearish_pct>=0&&flow.bearish_pct<=100){
  out.classifiedPct=100*classified/Math.max(classified+unknown,flow.contracts);
  if(out.classifiedPct>=65)out.bearishFlowPct=flow.bearish_pct;
 }
 const snapshot=s.optionSnapshotStatus;
 const horizon=snapshot?.horizons?.find(h=>h.mode==='DAY');
 const currentDay=!!(snapshot?.phase==='ready'&&horizon&&finite(horizon.observedMs)&&horizon.observedMs<=now&&now-horizon.observedMs<=300000&&dateET(horizon.observedMs)===dateET(now));
 const exposure=currentDay?horizon!.exposure:undefined;
 const gammaCovered=finite(exposure?.gammaUsed)&&exposure.gammaUsed>0&&finite(exposure.eligible)&&exposure.eligible>=exposure.gammaUsed;
 if(gammaCovered&&finite(exposure!.netGex)&&!exposure!.gexComplete){
  out.gexPartial=exposure!.netGex;
  out.gexCoverage=`${exposure!.gammaUsed}/${exposure!.eligible} contratos con gamma`;
 }
 if(gammaCovered&&exposure!.gexComplete&&finite(exposure!.netGex)){
  out.gex=exposure!.netGex;out.gexHorizon='DAY';
  out.gexRegime=out.gex>0?'POSITIVE':out.gex<0?'NEGATIVE':'NEUTRAL';
 }
 if(out.priceMovePct!=null&&out.volumeRatio!=null&&out.bearishFlowPct!=null){out.criteria=Number(out.priceMovePct<0)+Number(out.volumeRatio>=1.5)+Number(out.bearishFlowPct>=60);}
 return out;
}
const number=(n:number,d=2)=>n.toLocaleString('es-ES',{minimumFractionDigits:d,maximumFractionDigits:d});
export function quantFallPanel(s:LiveState|undefined,symbol:string,active:boolean,now:number):string {
 const r=quantFallReading(s,symbol,active,now),status=r.criteria==null?'DATOS PENDIENTES':`${r.criteria}/3 CRITERIOS OBSERVADOS`;
 const regime=r.gexRegime==='POSITIVE'?'GEX POSITIVO':r.gexRegime==='NEGATIVE'?'GEX NEGATIVO':r.gexRegime==='NEUTRAL'?'GEX NEUTRAL':r.gexPartial!=null?r.gexPartial>0?'GEX POSITIVO':r.gexPartial<0?'GEX NEGATIVO':'GEX SIN SIGNO':'GEX PENDIENTE';
 const partialSign=r.gexPartial==null?'':r.gexPartial>0?'positivo':r.gexPartial<0?'negativo':'cero';
 const gexDetail=r.gex!=null?`GEX neto ${number(r.gex,0)} USD por 1% · convención experimental`:r.gexPartial!=null?`PARCIAL OBSERVADO · ${r.gexCoverage} · subtotal ${partialSign} ${number(r.gexPartial,0)} USD por 1% · cobertura GEX incompleta; régimen total DAY sin confirmar`:'OI/gamma o captura vigente pendientes';
 const cell=(n:string,value:string,detail:string,tone:string)=>`<div class="quant-fall-cell ${tone}"><span>${n}</span><strong>${value}</strong><small>${detail}</small></div>`;
 return `<section class="structural-map-preview quant-fall-panel" aria-label="Lectura cuantitativa de movimiento"><div class="section-head"><strong>LECTURA CUANTITATIVA · MOVIMIENTO</strong><span>${esc(symbol)} · ${r.asOfMs?new Date(r.asOfMs).toLocaleTimeString('es-ES',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'}):'—'} ET</span></div><div class="quant-fall-status">${status}</div><div class="quant-fall-grid">${cell('1 · PRECIO · 3 MIN',r.priceMovePct==null?'—':`${r.priceMovePct>=0?'+':''}${number(r.priceMovePct)}%`,'Cierre actual frente al cierre de tres velas antes',r.priceMovePct!=null&&r.priceMovePct<0?'negative':'')}${cell('2 · VOLUMEN · 3 MIN',r.volumeRatio==null?'—':`${number(r.volumeRatio)}×`,'Media de 3 minutos ÷ media de los 10 previos',r.volumeRatio!=null&&r.volumeRatio>=1.5?'accent':'')}${cell('3 · OPCIONES · 5 MIN',r.bearishFlowPct==null?'—':`${number(r.bearishFlowPct,0)}% bajista`,'Flujo clasificado'+(r.classifiedPct==null?' pendiente':` · cobertura ${number(r.classifiedPct,0)}%`),r.bearishFlowPct!=null&&r.bearishFlowPct>=60?'negative':'')}${cell('4 · GAMMA PROPIA · DAY',regime,gexDetail,r.gexRegime==='NEGATIVE'?'negative':r.gexRegime==='POSITIVE'?'positive':'')}</div>${massiveExposurePanel(s,symbol,active,now)}<p class="quant-fall-note">Umbrales de observación provisionales: precio &lt; 0%, volumen ≥ 1,5×, flujo bajista ≥ 60% con ≥ 65% de contratos clasificados. Solo velas cerradas y datos del mismo activo. GEX = Σ(gamma × OI × multiplicador × precio² × 0,01) CALL − PUT; no mide inventario real de dealers. Sin probabilidad calibrada ni señal de orden.</p></section>`;
}
