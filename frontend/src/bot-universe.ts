import {botAssetNewsPanel} from './top-five-news';
import {scannerDealers,scannerDealerScore} from './scanner-dealers';
import {updatePanel} from './live-dom';
import type {NeuroContext} from './neuro-context';
import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
export interface BotUniverse {session:string;symbols:string[];verifiedMs:number|null;detail:string}
interface Portfolio {asset_mode?:'five'|'qqq';top5_connected?:boolean;armed?:boolean;reserved_total?:number;universe?:BotUniverse;asset_errors?:Record<string,string>;readings?:Record<string,{reading?:{eligible:boolean;entry_side:string;blockers:string[];flow_expiry?:string;neuro_context?:NeuroContext}}>;
 sessions?:{symbol:string;side?:string;phase:string;reserved_premium?:number;reason:string;entry_context?:{expiry_plan?:{expiry:string;reason:string}}}[]}
let current:Portfolio|undefined,busy=false,error='',pending=false;
const money=(v:number|undefined)=>v==null?'—':new Intl.NumberFormat('en-US',{style:'currency',currency:'USD'}).format(v);
export function portfolioPanel(value:Portfolio|undefined=current):string {
 const rows=value?.sessions?.filter(s=>s.phase!=='done')??[];
 return `<section class="panel bot-portfolio"><h3>Cartera estructural · ${rows.length} / 5 posiciones</h3><p>Máximo $1.000 por entrada · $5.000 en conjunto · una posición por activo</p><p>Prima reservada en posiciones y entradas pendientes: <b>${money(value?.reserved_total)}</b></p>${rows.map(s=>`<article class="bot-management"><h4>${esc(s.symbol)} · ${esc(s.side??'')}</h4><p>${esc(s.phase)} · reservado ${money(s.reserved_premium)} · vence ${esc(s.entry_context?.expiry_plan?.expiry??'consultar contrato')}</p><p>${esc(s.reason)}</p><p>${esc(s.entry_context?.expiry_plan?.reason??'')}</p></article>`).join('')||'<p>Sin posiciones abiertas del controlador.</p>'}${Object.entries(value?.asset_errors??{}).map(([symbol,reason])=>`<p role="status"><b>${esc(symbol)}</b>: ${esc(reason)}</p>`).join('')}</section>`;
}
export function botAssetMode(mode:'five'|'qqq'='five'):string {return `<div class="bot-asset-mode" aria-label="Activos del bot">${([['five','Cinco activos'],['qqq','Solo QQQ']] as const).map(([m,label])=>`<button data-bot-asset-mode="${m}" aria-pressed="${mode===m}">${label}</button>`).join('')}<small>Modo para nuevas entradas · las posiciones abiertas siguen bajo gestión.</small></div>`;}
export function botUniversePanel(selection:BotUniverse|undefined):string {
 const universe=selection??current?.universe;
 return `<section class="panel"><div class="panel-head"><div><h2>Cinco del bot · verificación 09:30 ET</h2><p>${esc(universe?.detail??'Conecta Massive para recuperar el premarket del universo.')}</p></div><button class="primary-button" data-connect-top5 ${busy||!isTauri()?'disabled':''}>${busy?'Conectando…':current?.top5_connected?'Bot conectado · segundo plano':'Conectar con el bot'}</button></div>${botAssetMode(current?.asset_mode)}<p>La selección del bot queda fija por volumen premarket. Cambiar la pantalla o el periodo del scanner no cambia estos cinco.</p><p>${current?.armed?'Entradas Paper activadas para la sesión autorizada.':'Análisis independiente. Activa las entradas Paper desde Snipers Control.'}</p>${error?`<p role="alert">${esc(error)}</p>`:''}<div class="bot-universe-cards scanner-cards">${[...(current?.asset_mode==='qqq'?['QQQ']:universe?.symbols??[])].sort((a,b)=>(scannerDealerScore(current?.readings?.[b]?.reading?.neuro_context,b,!!current?.top5_connected)??-1)-(scannerDealerScore(current?.readings?.[a]?.reading?.neuro_context,a,!!current?.top5_connected)??-1)).map(symbol=>{const r=current?.readings?.[symbol]?.reading;return `<article class="scanner-card"><h3>${esc(symbol)}</h3><p>${r?.eligible?`Señal ${esc(r.entry_side)} · verificando ejecución`:'Analizando estructura y flujo propios'}</p><small>Flujo del vencimiento ${esc(r?.flow_expiry??'pendiente')}</small>${scannerDealers(r?.neuro_context,symbol,!!current?.top5_connected)}${(r?.blockers??[]).slice(0,3).map(b=>`<p>${esc(b)}</p>`).join('')}${botAssetNewsPanel(symbol)}</article>`;}).join('')||'<p>Esperando cinco activos verificados.</p>'}</div></section>`;
}
export function bindBotUniverse(){
 document.querySelectorAll<HTMLButtonElement>('[data-bot-universe] [data-bot-asset-mode]').forEach(b=>b.onclick=async()=>{if(busy||!isTauri())return;busy=true;try{await invoke('set_paper_bot_asset_mode',{mode:b.dataset.botAssetMode});await poll();}catch(e){error=String(e);}finally{busy=false;render();}});
 const button=document.querySelector<HTMLButtonElement>('[data-connect-top5]');
 if(button)button.onclick=async()=>{if(busy)return;busy=true;error='';try{await invoke('open_neuro_trader');await invoke('connect_top_five_bot');await poll();}catch(e){error=String(e);}finally{busy=false;render();}};
}
function render(){const host=document.querySelector<HTMLElement>('[data-bot-universe]');if(host){updatePanel(host,botUniversePanel(current?.universe));bindBotUniverse();}}
async function poll(){if(pending||!isTauri()||!document.querySelector('[data-bot-universe]'))return;pending=true;try{current=await invoke<Portfolio>('get_paper_bot_state');}catch(e){error=String(e);}finally{pending=false;render();}}
setInterval(()=>{void poll();},2000);
