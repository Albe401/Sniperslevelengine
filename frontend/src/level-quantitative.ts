import {levelOpenInterest} from './level-open-interest';
import {sameStrike} from './strike-key';
import {barrierFlow} from './barrier-flow';
import type {EtfReport} from './etf-dealers';
import type {BidAskWindow} from './strike-bid-ask';
import type {MarketEvidence} from './market-evidence';
import type {LevelEvent} from './intraday-map';
export function levelDefense(strike:number,market:MarketEvidence|undefined,events:LevelEvent[]=[],now=Date.now()){
 const noEvidence={label:'Defensa no verificable',detail:'Sin precio y evidencia de mercado vigentes. La concentración de contratos no prueba defensa.'};
 if(!Number.isFinite(now)||!market||!Number.isFinite(market.at)||market.at>now||now-market.at>3000||!market.price.ready)return noEvidence;
 const event=events.find(e=>e.level===strike&&e.sustained&&Number.isFinite(e.at)&&e.at<=now&&now-e.at<=300000&&[1,-1].includes(e.direction)&&['Defensa observada','Rechazo observado'].includes(e.action));
 const c=market.cvd;
 if(!event)return {label:'Sin reacción sostenida',detail:'No hay una reacción sostenida vigente en este nivel; el OI por sí solo no confirma defensa.'};
 if(market.price.spot==null||!Number.isFinite(market.price.spot)||(market.price.spot-strike)*event.direction<=0)return {label:'Defensa sin confirmar',detail:'La reacción registrada no conserva el precio del lado esperado del nivel.'};
 if(c.signal==null||!Number.isFinite(c.signal)||Math.abs(c.signal)>1||c.coverage==null||!Number.isFinite(c.coverage)||c.coverage<80||c.coverage>100||c.at==null||!Number.isFinite(c.at)||c.at>now||now-c.at>3000)return {label:'Reacción sin CVD comparable',detail:'Hay una reacción sostenida, pero falta CVD de 5 minutos vigente con al menos 80% de volumen clasificado; no confirma defensa.'};
 if(Math.abs(c.signal)<.1)return {label:'Defensa sin presión clara',detail:'Interacción sostenida, pero el CVD global no tiene predominio suficiente.'};
 if(Math.sign(c.signal)!==event.direction)return {label:'Defensa contradicha',detail:'La reacción del nivel y el CVD global van en sentidos opuestos; no confirma defensa.'};
 return {label:event.direction>0?'Defensa compradora observada':'Defensa vendedora observada',detail:`Reacción sostenida del precio y CVD 5 min alineado, cobertura ${c.coverage.toFixed(1)}%. El CVD es del activo, no atribuible a órdenes de ese strike. Lectura provisional; no identifica quién defiende ni garantiza continuidad.`};
}
/** Composition and sampled contract pressure; neither dealer inventory nor calibrated probability. */
export function quantitativeLevel(strike:number,report:EtfReport|undefined,rows:BidAskWindow[]|undefined,expirations:string[],fresh:boolean){
 const oi=levelOpenInterest(report,strike,expirations),flow=barrierFlow(rows,strike,expirations,fresh);
 let bullish:number|null=null,bearish:number|null=null,imbalance:number|null=null;
 if(flow&&flow.coverage>=80){
  const selected=rows!.filter(r=>sameStrike(r.strike,strike)&&expirations.includes(r.expiration)&&['call','put'].includes(r.kind));
  bullish=selected.reduce((n,r)=>n+(r.kind==='call'?r.ask!:r.bid!),0);
  bearish=selected.reduce((n,r)=>n+(r.kind==='call'?r.bid!:r.ask!),0);
  const known=bullish+bearish;
  if(Number.isFinite(known)&&known>0)imbalance=(bullish-bearish)/known;else {bullish=null;bearish=null;}
 }
 const label=imbalance==null?'Presión no verificable':Math.abs(imbalance)<.1?'Presión equilibrada':imbalance>0?'Presión alcista inferida':'Presión bajista inferida';
 const oiSide=oi?(oi.zeroOi?'OI CALL 0 · PUT 0; participación porcentual indefinida sin contratos abiertos.':`OI CALL ${oi.call} (${oi.callPct}%) · PUT ${oi.put} (${oi.putPct}%).`):'OI por lado sin cobertura verificada.';
 const detail=[`${oiSide} ${oi?oi.verification==='reference'?'Celdas cotejadas con el mapa de OI; población estándar de 100 acciones sin entregables adicionales; ajustados excluidos.':'Suma local del snapshot sin referencia independiente.':''}`,
  imbalance==null?'Presión no evaluable: necesita ventana vigente y ≥80% de volumen clasificado.':`${label}: ${(100*(imbalance+1)/2).toFixed(1)}% alcista / ${(100*(1-imbalance)/2).toFixed(1)}% bajista; desequilibrio ${imbalance.toFixed(3)}.`,
  flow?`Cobertura clasificada ${flow.coverage.toFixed(1)}% de la muestra · sin clasificar ${flow.unknown} contratos.`:'Sin ventana de operaciones cubierta.',
  '5 min · strike exacto · contratos de la muestra suscrita. Alcista = CALL ASK + PUT BID; bajista = CALL BID + PUT ASK. Desequilibrio = (alcista − bajista)/(alcista + bajista). ASK/BID infiere agresor, no apertura/cierre ni inventario dealer.',
  'La fuerza estructural es concentración, no presión activa. No se calcula probabilidad de llegada/rechazo/ruptura sin calibración histórica; no se deduce de OI ni de este porcentaje.'].join(' ');
 return {oi,flow,bullish,bearish,imbalance,label,detail,probability:null};
}
