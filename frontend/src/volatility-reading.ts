import type {ControlReading} from './intraday-control';
import {absorptionReading} from './absorption-reading';
import {rangeFresh} from './daily-range';
import {escapeHtml as esc} from './model';
import './volatility-reading.css';

export function volatilityReading(r:ControlReading,now=Date.now()) {
 const pending={tone:'waiting',headline:'ESPERANDO LECTURA DE VOLATILIDAD',detail:'Todavía falta evidencia vigente para interpretar la volatilidad y su dirección acompañante.'};
 const observation=r.ivObservation,iv=r.iv,p=r.priceNow,f=r.flowControl;
 if(!observation||!Number.isFinite(observation.percent)||observation.percent<=0)return {...pending,detail:'Falta una observación válida de IV ATM del snapshot de opciones.'};
 if(!rangeFresh({phase:'ready',observedMs:observation.at},now))return {...pending,detail:'La observación de IV ATM no está vigente; esperando actualización del snapshot.'};
 if(!iv||iv.delta==null||!Number.isFinite(iv.delta))return {...pending,detail:'Esperando una segunda observación comparable de IV ATM para medir su cambio.'};
 if(iv.trend==='CONTRAYENDO')return {tone:'neutral',headline:'VOLATILIDAD CONTRAYENDO · ESPERAR',detail:'La volatilidad implícita está bajando; no hay expansión que acompañar en esta lectura.'};
 if(iv.trend!=='EXPANDIENDO')return {tone:'neutral',headline:'VOLATILIDAD ESTABLE · ESPERAR',detail:'Todavía no aparece una expansión clara de volatilidad.'};
 if(!p?.ready)return {tone:'neutral',headline:'VOLATILIDAD SUBIENDO · DIRECCIÓN SIN CONFIRMAR',detail:'La IV está expandiéndose; faltan precio y velas vigentes para confirmar una dirección CALL o PUT.'};
 const absorption=absorptionReading(r,now);
 if(absorption.kind!=='waiting')return {tone:'neutral',headline:'VOLATILIDAD SUBIENDO · POSIBLE FRENO',detail:absorption.kind==='fall'?'Hay indicios de absorción de la caída. La continuidad hacia PUT está en duda.':'Hay indicios de absorción de la subida. La continuidad hacia CALL está en duda.'};
 const direction=r.marketEvidence?r.marketEvidence.direction:p.direction,side=direction===1?'CALL':'PUT';
 const aligned=direction!==0&&direction!=null&&f?.quality==='ready'&&f.tone===(direction===1?'buyers':'sellers')&&p.structure===(direction===1?'MÁXIMOS Y MÍNIMOS MÁS ALTOS':'MÁXIMOS Y MÍNIMOS MÁS BAJOS')&&p.vwap.startsWith(direction===1?'SOBRE VWAP':'BAJO VWAP')&&['ACELERANDO EN VELAS CERRADAS',direction===1?'SUBIDA SOSTENIDA':'CAÍDA SOSTENIDA'].includes(p.pace);
 if(aligned)return {tone:direction===1?'call':'put',headline:`VOLATILIDAD SUBIENDO · FAVORECE ${side}`,detail:direction===1?'La expansión acompaña un impulso alcista sostenido. El escenario favorece continuidad hacia CALL; aún requiere una entrada confirmada.':'La expansión acompaña un impulso bajista sostenido. El escenario favorece continuidad hacia PUT; aún requiere una entrada confirmada.'};
 return {tone:'neutral',headline:'VOLATILIDAD SUBIENDO · DIRECCIÓN SIN CONFIRMAR',detail:'La volatilidad está expandiéndose, pero el movimiento todavía no permite favorecer CALL o PUT con evidencia alineada.'};
}
export function intradayPressure(r:ControlReading) {
 if(r.marketEvidence){const e=r.marketEvidence;return {value:e.direction&&e.score.call!=null?(e.score.call-50)/50:0,available:e.score.call!=null,partial:e.score.call==null};}
 const signals=(r.components??[]).filter(c=>['FLOW 1M','FLOW 5M','CALL BUY / PUT BUY 1M'].includes(c.name)&&Number.isFinite(c.signal)&&Number.isFinite(c.weight)&&c.weight>0);
 const flowWeight=signals.reduce((sum,c)=>sum+c.weight,0);
 const flow=flowWeight?signals.reduce((sum,c)=>sum+c.signal*c.weight,0)/flowWeight:undefined;
 const price=Number.isFinite(r.pricePressure)?r.pricePressure:undefined;
 const weight=(flow==null?0:60)+(price==null?0:40);
 const value=weight?Math.max(-1,Math.min(1,((flow??0)*60+(price??0)*40)/weight)):0;
 return {value,available:weight>0,partial:flow==null||price==null||r.flowControl?.quality!=='ready'};
}
export function volatilityReadingPanel(r:ControlReading,now=Date.now()):string {
 const reading=volatilityReading(r,now);
 const pressure=intradayPressure(r),direction=pressure.value;
 const tone=!pressure.available?'waiting':direction>0?'call':direction<0?'put':'neutral';
 const headline=!pressure.available?'PRESIÓN INTRADÍA · ESPERANDO PRECIO O FLUJO':`PRESIÓN INTRADÍA · ${direction>0?'CALL':direction<0?'PUT':'EQUILIBRIO'} · ${Math.round(Math.abs(direction)*100)}/100${pressure.partial?' · PARCIAL':''}`;
 const position=50+direction*50,fill=Math.abs(direction)*50,start=direction<0?position:50;
 const observation=r.ivObservation;
 const valid=observation&&rangeFresh({phase:'ready',observedMs:observation.at},now)&&Number.isFinite(observation.percent)&&observation.percent>0;
 const ivLabel=valid?'IV ATM '+observation.percent.toFixed(2)+' %':'IV ATM pendiente';
 const delta=valid?r.iv?.delta:null;
 const change=delta!=null&&Number.isFinite(delta)?' · cambio '+(delta>=0?'+':'')+delta.toFixed(2)+' puntos de IV':'';
 return `<section class="volatility-reading volatility-${tone}" aria-label="Presión intradía: PUT izquierda, CALL derecha"><div class="volatility-meter" title="${esc(headline+' — '+reading.headline+' — '+reading.detail)}" role="meter" aria-label="${esc(headline)}" aria-valuemin="-1" aria-valuemax="1" aria-valuenow="${direction}" aria-valuetext="${esc(headline+'. '+reading.headline+'. '+reading.detail)}"><i class="volatility-arrow volatility-arrow-put" aria-hidden="true"></i><div class="volatility-track" aria-hidden="true"><div class="volatility-fill" style="left:${start}%;width:${fill}%"></div><i class="volatility-center"></i><i class="volatility-marker" style="left:${position}%"></i></div><i class="volatility-arrow volatility-arrow-call" aria-hidden="true"></i></div><div class="volatility-status" role="status"><strong>${esc(headline)}</strong><span>${esc(reading.headline)}</span><span>${esc(ivLabel+change)}</span><p>${esc(reading.detail)}</p><small>Presión: flujo 60 % y precio 40 %, normalizados según evidencia disponible. Precio frente al último cierre de 2 min; escala de 0,1 %. Índice descriptivo, no probabilidad ni señal de entrada.</small></div></section>`;
}
