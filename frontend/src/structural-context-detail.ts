import {escapeContextHtml as esc} from './structural-context-html';
import type {StructuralMapContext} from './structural-map-context';
import type {PriceStructureCard} from './price-structure';
const stamp=(n:number|null)=>n==null?'sin fecha verificable':new Intl.DateTimeFormat('es-ES',{timeZone:'America/New_York',day:'2-digit',month:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(n)+' ET';
const price=(n:number|null)=>n==null?'sin referencia verificable':'$'+n.toFixed(2);

/** The same validation used by the source detail also gates visible cards. */
export function connectedStructureCards(cards:PriceStructureCard[],context:StructuralMapContext):PriceStructureCard[] {
 return cards.map(card=>{
  const lane=context.price.find(c=>c.interval===card.interval);
  const accepted=lane?.status==='available'&&lane.side===card.side&&lane.closedAtMs===card.endMs||lane?.status==='pending'&&card.side==='SIN DATOS';
  return accepted?card:{...card,side:'SIN DATOS',reason:card.side==='SIN DATOS'&&card.reason?card.reason:lane?.reason??'Sin asociación verificable para esta temporalidad',highs:[],lows:[],highChange:null,lowChange:null,atr:null,lastClose:null,endMs:null};
 });
}

/** Fits inside the existing detail dialog; no extra rows in the six-card grid. */
export function structuralContextDetail(c:StructuralMapContext):string {
 const o=c.options,usable=c.price.filter(p=>p.status==='available').length;
 const options=o.profile?`GEX Massive: ${o.mode==='DAY'?'vencimiento del día':'próximo vencimiento'} ${o.expiry}; captura ${stamp(o.observedMs)}${o.freshness==='historical'?' · histórica':''}. ${o.status==='partial'?'Subtotal parcial':'Población recibida'}: ${o.gammaUsed}/${o.eligible} contratos con gamma. Precio de referencia de opciones: ${price(o.spot)}.`:`GEX Massive: ${o.reason}.`;
 const activity=Object.values(c.activity).map((v,i)=>`<p><b>${i===0?'Flujo de opciones':'CVD de acciones'} · 5 minutos:</b> ${esc(v.reason)}. Se conserva como contexto reciente con su propia ventana.</p>`).join('');
 const names:Record<string,string>={DAY:'hoy',NEXT_SESSION:'próximo vencimiento',WEEK:'semana',NEXT4:'próximos cuatro vencimientos'};
 const overlaps=c.optionScopes.filter(s=>s.overlaps.length).map(s=>names[s.mode]??'otro horizonte de opciones');
 return `<details data-horizon-details="structural-connections" data-structural-context-version="${c.version}"><summary>Datos conectados y cobertura</summary><p><b>${esc(c.symbol)}:</b> ${usable}/5 temporalidades con datos estructurales verificables. Cada tarjeta conserva su cierre y su ATR.</p><p${o.id?` data-options-context-id="${esc(o.id)}"`:''}>${esc(options)}</p><p>Las tarjetas consultan el mismo contexto GEX; no son cinco confirmaciones independientes. Un neto de gamma negativo no decide PUT. El precio de opciones y el cierre de las velas conservan sus referencias propias; la fecha de captura no acredita la fecha económica del OI ni la hora del trade de ese precio.</p>${activity}<p><b>Flujo por strike:</b> ${esc(c.strikeFlow.reason)}.</p>${overlaps.length?`<p>Los horizontes ${esc(overlaps.join(', '))} comparten algunos vencimientos. Sus cantidades no se suman como poblaciones independientes.</p>`:''}</details>`;
}
