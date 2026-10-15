import {deriveView,magnet} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/analytics';
import {dealerIntradayPanel} from './dealer-intraday';
import type {NeuroContext} from './neuro-context';
import {escapeHtml as esc} from './model';
import './scanner-dealers.css';
import {invoke,isTauri} from './snapshot-observation';
let serviceStarted=false;
export function ensureScannerNeuro(){if(serviceStarted||!isTauri())return;serviceStarted=true;void invoke('open_neuro_trader').catch(()=>{serviceStarted=false;});}
export function scannerDealerScore(c:NeuroContext|undefined,symbol:string,active:boolean,now=Date.now()):number|null {
 if(!active||!c?.available||c.symbol!==symbol||c.at>now||now-c.at>45000||c.modelPayload?.symbol!==symbol)return null;
 try {const v=deriveView(c.modelPayload,true,'std'),m=magnet(c.modelPayload,v);
  // Context priority measures directional separation and usable liquidity, not win probability.
  const n=0.6*Math.abs(v.sc.bias-50)*2+0.4*m.strength;
  return Number.isFinite(n)?Math.max(0,Math.min(100,n)):null;
 }catch{return null;}
}
export function scannerDealers(c:NeuroContext|undefined,symbol:string,active:boolean,now=Date.now()):string {
 const priority=scannerDealerScore(c,symbol,active,now);
 let head='Contexto pendiente',color='#ffc24b';
 if(priority!=null) {const v=deriveView(c!.modelPayload!,true,'std');head=v.sc.bias>=60?'Sesgo CALL':v.sc.bias<=40?'Sesgo PUT':'Sesgo neutral';color=v.sc.bias>=60?'#35f2a1':v.sc.bias<=40?'#ff3d6e':'#ffc24b';}
 return `<div class="scanner-dealers"><p style="color:${color}"><b>${esc(head)}</b> · prioridad ${priority==null?'—':Math.round(priority)+'%'}</p>${dealerIntradayPanel(c,symbol,active,now)}</div>`;
}
