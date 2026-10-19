import {drawingState,drawingToolbar,drawingsSvg} from './chart-drawings';
import type {SessionCandle} from './candle-series';
import {chart,type ChartMark} from './chart';
import {escapeHtml as esc,price,aggregate,type Candle} from './model';
import type {Horizon} from './live';
const money=(n:number)=>Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
export function gexMap(symbol:string,h:Horizon|undefined,fresh:boolean,source:Candle[],interval:number,now=Date.now(),options:{prepared?:boolean;date?:string;visible?:number;panOffset?:number;before?:number;marks?:ChartMark[];vwap?:boolean;height?:number;priceRange?:[number,number]}={}):string {
 const date=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const valid=fresh&&!!h?.exposure.gexComplete;
 const observed=!!h&&Number.isFinite(h.observedMs)&&h.observedMs>0&&h.observedMs<=now;
 const received=observed&&(h!.exposure.gammaUsed>0||h!.exposure.gexComplete)?h!.exposure:undefined;
 let e=fresh?received:undefined;
 const all=(options.prepared?source:aggregate(source.filter(c=>Number.isFinite(c.timestampMs)&&c.timestampMs<=now&&date(c.timestampMs)===date(now)&&[c.open,c.high,c.low,c.close].every(p=>Number.isFinite(p)&&p>0)&&c.high>=Math.max(c.open,c.close)&&c.low<=Math.min(c.open,c.close)),interval)) as SessionCandle[];
 const selectedDate=options.date||(options.prepared?all.at(-1)?.sessionDate:undefined);
 const eligible=all.filter(c=>(!selectedDate||c.sessionDate===selectedDate)&&(!options.before||c.timestampMs<=options.before));
 const candles=eligible;
 const count=Math.max(8,Math.min(eligible.length,options.visible??180));
 const viewport={start:Math.max(0,eligible.length-count)+(options.panOffset??0),count};
 const visibleCandles=candles.filter((_,i)=>i>=viewport.start-1&&i<=viewport.start+count);
 const historical=!!candles.length&&date(candles.at(-1)!.timestampMs)!==date(now);

 if(historical)e=undefined;
 const center=candles.at(-1)?.close??e?.spot;
 const profileRows=received?.strikes.filter(r=>Number.isFinite(r.strike)&&r.strike>0&&Number.isFinite(r.netGex)&&Number.isFinite(received.spot)&&received.spot>0&&Math.abs(r.strike/received.spot-1)<=.05).sort((a,b)=>b.strike-a.strike)??[];
 const peakProfile=Math.max(1,...profileRows.map(r=>Math.abs(r.netGex)));
 const profileState=!fresh?'ÚLTIMA CONSULTA · NO VIGENTE':received?.gexComplete?'COBERTURA COMPLETA':'COBERTURA PARCIAL · SIN RÉGIMEN CONFIRMADO';
 const profile=profileRows.length?`<section class="gex-received-profile" aria-label="Exposición gamma recibida"><header><div><strong>GAMMA POR STRIKE</strong><small>${esc(profileState)}</small></div><span>${h?.exposure.gammaUsed??'—'} / ${h?.exposure.eligible??'—'} contratos con gamma</span></header><p>Consulta ${esc(new Date(h!.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))} ET · referencia ${price(received!.spot)} · vencimientos ${esc(h!.expirations.join(', '))}.${historical?' Las velas son de otra sesión: este perfil no representa el gamma histórico de esas velas.':''}</p><div class="gex-profile-rows">${profileRows.map(r=>`<div class="gex-profile-row" style="--gex-color:${r.netGex>=0?'#28d8d0':'#d96bdf'}"><b>${price(r.strike)}</b><span class="gex-profile-track"><i style="width:${Math.abs(r.netGex)/peakProfile*100}%"></i></span><strong>${r.netGex>0?'+':''}${money(r.netGex)}</strong></div>`).join('')}</div><small>GEX neto recibido · USD / movimiento del 1% · muestra ±5% de la referencia. ${received!.gexComplete?'':'Faltan contratos: estos valores son parciales; no confirman muros ni zero gamma.'}</small></section>`:`<div class="gex-data-message">Gamma no disponible: ${!h?'todavía no se recibió la cadena de opciones. Conecta Data en Mercado en vivo.':!observed?'la consulta no tiene una fecha válida.':'no hay strikes con gamma utilizable en el rango de referencia.'}</div>`;
 const strikes=(!historical?e:undefined)?.strikes.filter(r=>Number.isFinite(r.strike)&&r.strike>0&&Number.isFinite(r.netGex)&&center!=null&&Math.abs(r.strike/center-1)<=.05)??[];
 const levels:{label:string;value:number;color:string}[]=[];
 const add=(label:string,value:number|null|undefined,color:string)=>{if(value!=null&&Number.isFinite(value)&&value>0)levels.push({label,value,color});};
 if(valid&&!historical){add('ZERO GAMMA',e?.zeroGamma,'#f0c869');if(h?.structure.complete){add('CALL WALL',h.structure.callWall?.strike,'#35dfad');add('PUT WALL',h.structure.putWall?.strike,'#ff6480');}}
 const visibleLevels=levels.filter(l=>center!=null&&Math.abs(l.value/center-1)<=.05);
 const values=[...visibleCandles.flatMap(c=>[c.high,c.low]),...strikes.map(r=>r.strike),...visibleLevels.map(l=>l.value),...visibleCandles.flatMap(c=>options.vwap!==false&&c.sessionVwap!=null?[c.sessionVwap]:[])];
 let svg='<div class="gex-empty">Esperando velas del activo y exposición GEX vigente.<small>Conecta Data en Mercado en vivo. No se muestran datos de ejemplo.</small></div>';
 if(candles.length){
 svg=chart({symbol,name:symbol,previousClose:candles[0].open,candles,levels:[]},candles.length,interval,false,'live',options.marks??[],{
  prepared:true,gex:true,viewport,height:options.height,scalePrices:values,priceRange:options.priceRange,
  candleDetail:i=>`${candles[i].sessionDate??date(candles[i].timestampMs)} · ${candles[i].complete?'Cerrada':'Parcial / cobertura incompleta'} · VWAP ${candles[i].sessionVwap==null?'—':price(candles[i].sessionVwap!)}`,
  overlay:(geometry)=>{
   const {left,end,candleEnd,top,floor,x,y}=geometry;
   const drawing=drawingState(symbol);
   const annotations=`<defs><clipPath id="gex-price-clip"><rect x="${left}" y="${top}" width="${end-left}" height="${floor-top}"/></clipPath><clipPath id="gex-user-clip"><rect x="${left}" y="${top}" width="${candleEnd-left}" height="${floor-top}"/></clipPath></defs><g data-user-drawings clip-path="url(#gex-user-clip)">${drawing.hidden?'':drawingsSvg(drawing.rows,drawing.selected,geometry,candles.map(c=>c.timestampMs))}</g>`;
   const peak=Math.max(1,...strikes.map(r=>Math.abs(r.netGex)));
   const sorted=[...strikes].sort((a,b)=>a.strike-b.strike);
   const gaps=sorted.slice(1).map((r,i)=>Math.abs(y(r.strike)-y(sorted[i].strike)));
   const height=Math.max(1,Math.min(12,(gaps.length?Math.min(...gaps):16)*.65));
   const strongest=[...strikes].sort((a,b)=>Math.abs(b.netGex)-Math.abs(a.netGex)).slice(0,3);
   const bars=strikes.map(r=>{
    const size=Math.abs(r.netGex)/peak*(end-candleEnd-28),color=r.netGex>=0?'#28d8d0':'#d96bdf';
    const dominant=strongest.includes(r);
    return `<g class="gex-strike"><title>Strike ${price(r.strike)} · GEX ${money(r.netGex)} USD / 1%</title><rect x="${left}" y="${y(r.strike)-height/2}" width="${candleEnd-left}" height="${height}" fill="${color}" opacity="${dominant?.14:.04}"/><rect x="${end-size}" y="${y(r.strike)-height/2}" width="${size}" height="${height}" rx="2" fill="${color}"/>${dominant?`<text x="${candleEnd+20}" y="${y(r.strike)-height/2-4}" fill="${color}" font-size="10">${price(r.strike)} · ${money(r.netGex)}</text>`:''}</g>`;
   }).join('');
   const walls=visibleLevels.map(l=>`<g><title>${l.label} ${price(l.value)}</title><line x1="${left}" x2="${candleEnd}" y1="${y(l.value)}" y2="${y(l.value)}" stroke="${l.color}" stroke-dasharray="5 4"/><text x="${left+6}" y="${y(l.value)-7}" fill="${l.color}" font-size="11" font-weight="600">${l.label} ${price(l.value)}</text></g>`).join('');
   let vwap='';let points:string[]=[];
   const flush=()=>{if(points.length>1)vwap+=`<polyline points="${points.join(' ')}" fill="none" stroke="#f0c869" stroke-width="1.8"/>`;points=[];};
   if(options.vwap!==false) candles.forEach((c,i)=>{const prev=candles[i-1];if(c.sessionVwap==null||prev&&(prev.sessionDate!==c.sessionDate||c.timestampMs-prev.timestampMs>interval*60000))flush();if(c.sessionVwap!=null)points.push(`${x(i)},${y(c.sessionVwap)}`);});flush();
   return `<text x="${left}" y="18" fill="#8ecfff" font-size="11">PRECIO · ${interval}m · ${historical?'HISTÓRICO':'SESIÓN'} · ET</text><text x="${end}" y="18" fill="#8ecfff" font-size="10" text-anchor="end">GEX / STRIKE · ±5%</text><g clip-path="url(#gex-price-clip)">${bars}${walls}<g clip-path="url(#gex-user-clip)">${vwap}</g></g>${annotations}<line x1="${candleEnd+10}" x2="${candleEnd+10}" y1="${top}" y2="${floor}" stroke="#339dff40"/><text x="${left}" y="${floor+46}" fill="#8ecfff" font-size="10">VOLUMEN NEGOCIADO</text>`;
  }
 });
 }

 return `<section class="panel gex-map"><div class="panel-head"><div><span class="eyebrow">${esc(symbol)} · ${esc(h?.label??'Horizonte DAY')}</span><h1>Mapa GEX</h1></div><span class="tag">${historical?'VELAS HISTÓRICAS · GEX ACTUAL OCULTO':valid?'SNAPSHOT GEX VIGENTE':e?'GEX PARCIAL · SIN CONFIRMAR':'GEX PENDIENTE O INCOMPLETO'}</span></div><div class="gex-summary"><span>GEX neto <b>${e?money(e.netGex):'—'}</b></span><span>Zero gamma <b>${!valid||e?.zeroGamma==null?'—':price(e.zeroGamma)}</b></span><span>Régimen <b>${valid&&e?esc(e.gammaRegime):'Sin confirmar'}</b></span><span>Último cierre ${interval}m <b>${candles.length?price(candles.at(-1)!.close):'—'}</b></span></div>${drawingToolbar(symbol,options.height??560)}<div class="gex-canvas" style="height:${options.height??560}px">${svg}</div>${profile}<div class="gex-cursor" data-gex-cursor>Mueve el cursor para consultar hora y precio · arrastra para navegar</div><div class="gex-legend"><span>Cian: GEX neto positivo</span><span>Magenta: GEX neto negativo</span><span>Amarillo continuo: VWAP · discontinuo: zero gamma</span></div><p class="panel-caption">Barras: magnitud de GEX neto en USD por movimiento del 1%, dentro de ±5% del último cierre disponible o referencia del snapshot. No es volumen entrante. Modelo call positivo / put negativo, no inventario observado de dealers. ${valid&&h?`Snapshot ${esc(new Date(h.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))} ET · vencimientos ${esc(h.expirations.join(', '))}.`:e?'Exposición parcial: se dibuja únicamente la gamma recibida, sin régimen confirmado.':'Sin exposición vigente sobre las velas; consulta el estado del perfil recibido.'} Los niveles actuales se dibujan sobre las velas como referencia; no muestran cómo era el GEX en cada minuto. Hasta cinco sesiones reales del motor. ${visibleCandles.length} velas en la ventana; la última puede estar en formación. VWAP ponderado con el dato del proveedor, reiniciado por sesión y oculto tras huecos de cobertura. Sin barras inventadas para huecos de datos.</p></section>`;
}
