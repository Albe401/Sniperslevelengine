import type {LiveState} from './live';
import {interpretationFresh} from './confirmations';
import {escapeHtml as esc,price} from './model';

export function selectedPrice(symbol:string,state:Pick<LiveState,'symbol'|'ticks'|'chartHistory'|'stocks'|'publishedMs'>|undefined,active:boolean,now=Date.now()) {
  const empty={symbol,value:null as number|null,at:null as number|null,live:false,label:active?'Cargando precio':'Sin datos · Mercado en vivo'};
  if(!state||state.symbol!==symbol)return empty;
  const tick=state.ticks[symbol];
  const candidates:{value:number;at:number;kind:'trade'|'close'}[]=[];
  if(tick?.symbol===symbol&&tick.price!=null&&Number.isFinite(tick.price)&&tick.price>0&&tick.tradeMs!=null&&Number.isFinite(tick.tradeMs)&&tick.tradeMs>0&&tick.tradeMs<=now+1000)
    candidates.push({value:tick.price,at:tick.tradeMs,kind:'trade'});
  for(const bar of state.chartHistory?.minutes??[]) {
    const at=bar.timestampMs+60000;
    if(Number.isFinite(at)&&bar.timestampMs>0&&at<=now&&Number.isFinite(bar.close)&&bar.close>0)
      candidates.push({value:bar.close,at,kind:'close'});
  }
  const latest=candidates.sort((a,b)=>b.at-a.at||(a.kind==='trade'?-1:1))[0];
  if(!latest)return empty;
  const live=latest.kind==='trade'&&active&&state.stocks.phase==='streaming'&&now-latest.at<=15000&&interpretationFresh(state.publishedMs,now);
  return {symbol,value:latest.value,at:latest.at,live,label:live?'En vivo':latest.kind==='trade'?'Última operación':'Cierre de vela · 1 min'};
}

export function selectedPriceMarkup(quote:ReturnType<typeof selectedPrice>,compact=false):string {
  const time=quote.at==null?'':new Date(quote.at).toLocaleString('es',{timeZone:'America/New_York',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false})+' ET';
  if(compact)return `<span class="spot-symbol">${esc(quote.symbol)}</span><strong title="${esc(quote.label)}${time?' · '+esc(time):''}">${quote.value==null?'—':'$'+price(quote.value)}</strong>`;
  return `<span class="spot-symbol">${esc(quote.symbol)}</span><strong>${quote.value==null?'—':'$'+price(quote.value)}</strong><span class="spot-detail" data-price-live="${quote.live}">${esc(quote.label)}${time?` · ${esc(time)}`:''}</span>`;
}
