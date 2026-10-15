import type {EtfReport} from './etf-dealers';
import {escapeHtml as esc} from './model';
const positive=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v)&&v>0;
const value=(v:number|null|undefined)=>v!=null&&Number.isFinite(v)&&v>=0?v.toFixed(2):'—';
export function topFiveAmplitude(r:EtfReport|undefined,now=Date.now()):string{
 const m=r?.dailyRange;
 const valid=!!r&&r.at<=now&&now-r.at<=300000&&m?.ready&&positive(m.spot)&&Math.abs(m.spot-r.spot)<1e-6&&positive(m.expectedMoveDollars)&&positive(m.expectedMovePct)&&Math.abs(m.expectedMovePct-100*m.expectedMoveDollars/m.spot)<.01;
 if(!valid)return '<p class="top-five-amplitude"><small>AMPLITUD DIARIA · CALL — · PUT — · esperando IV ATM válida</small></p>';
 const a=r.amplitude,anchored=!!a&&a.symbol===r.symbol&&a.available&&a.anchored&&a.at<=now&&now-a.at<=120000;
 const call=anchored?a.callDollars:m.expectedMoveDollars,put=anchored?a.putDollars:m.expectedMoveDollars;
 const callPct=anchored?a.callPct:m.expectedMovePct,putPct=anchored?a.putPct:m.expectedMovePct;
 return `<p class="top-five-amplitude"><small>${anchored?'RESTANTE DESDE APERTURA RTH':'AMPLITUD DIARIA · SNAPSHOT'}<br><span class="is-up">CALL +$${value(call)} · ${value(callPct)}%</span> · <span class="is-down">PUT −$${value(put)} · ${value(putPct)}%</span><br>Referencia $${value(anchored?a.reference:m.spot)} · banda $${value(anchored?a.lower:m.snapshotLower)}–$${value(anchored?a.upper:m.snapshotUpper)} · estimación, puede superarse${anchored&&(a.extensionCall??0)+(a.extensionPut??0)>0?' · fuera de banda':''}${r.enrichment?.priceFresh?'':' · precio antiguo, informativo'} · ${esc(m.method??'modelo nativo')}</small></p>`;
}
