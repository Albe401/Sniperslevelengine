import {escapeHtml as esc} from './model';
export interface BidAskWindow {ticker:string;kind:string;strike:number;expiration:string;covered:boolean;bid:number|null;ask:number|null;unknown:number|null}
export interface StrikeFlow {strike:number;call:number|null;put:number|null;callFlow:Amounts|null;putFlow:Amounts|null}
interface Amounts {bid:number;ask:number;unknown:number;series:number}
/** A complete window for sampled contracts is not coverage of the whole chain. */
export function strikeBidAsk(rows:BidAskWindow[]|undefined,expirations:string[],strikes:number[],fresh:boolean):StrikeFlow[]{
 const out=new Map(strikes.map(strike=>[strike,{strike,call:null,put:null,callFlow:null,putFlow:null} as StrikeFlow]));
 if(!fresh)return [...out.values()];
 const groups=new Map<string,BidAskWindow[]>();
 for(const r of rows??[]){if(!out.has(r.strike)||!expirations.includes(r.expiration)||!['call','put'].includes(r.kind))continue;const key=r.strike+':'+r.kind;const group=groups.get(key);if(group)group.push(r);else groups.set(key,[r]);}
 for(const rs of groups.values()){
  if(new Set(rs.map(r=>r.ticker)).size!==rs.length||rs.some(r=>!r.covered||[r.bid,r.ask,r.unknown].some(v=>v==null||!Number.isFinite(v)||v<0)))continue;
  const a=rs.reduce<Amounts>((a,r)=>({bid:a.bid+r.bid!,ask:a.ask+r.ask!,unknown:a.unknown+r.unknown!,series:a.series+1}),{bid:0,ask:0,unknown:0,series:0});
  const row=out.get(rs[0].strike)!;if(rs[0].kind==='call'){row.callFlow=a;row.call=a.bid+a.ask;}else{row.putFlow=a;row.put=a.bid+a.ask;}
 }
 return [...out.values()];
}
export function bidAskSvg(rows:(StrikeFlow&{y:number})[],height:number,headerY:number):string{
 const n=(v:number)=>new Intl.NumberFormat('es-ES',{maximumFractionDigits:0}).format(v);
 const peak=Math.max(1,...rows.flatMap(r=>[r.call??0,r.put??0]));
 const side=(a:Amounts|null,y:number,call:boolean)=>{
  const center=call?232:158;
  if(!a)return `<text x="${call?287:102}" y="${y+4}" text-anchor="middle" fill="#a6c7e4">—</text>`;
  const width=(a.ask+a.bid)/peak*112,ask=a.ask/peak*112,bid=a.bid/peak*112;
  return `<rect x="${call?center:center-ask}" y="${y-9}" width="${ask}" height="18" fill="#17d6a2"/><rect x="${call?center+ask:center-width}" y="${y-9}" width="${bid}" height="18" fill="#ff6273"/><text x="${call?386:6}" y="${y+4}" text-anchor="${call?'end':'start'}" paint-order="stroke" stroke="#061326" stroke-width="3" fill="#fff" font-size="10">A ${n(a.ask)} / B ${n(a.bid)}</text>`;
 };
 return `<text x="102" y="${headerY}" text-anchor="middle" fill="#ff6273" font-size="10">PUT</text><text x="287" y="${headerY}" text-anchor="middle" fill="#17d6a2" font-size="10">CALL</text>`+rows.map(r=>{
  const detail=(a:Amounts|null)=>a?`ASK ${n(a.ask)} · BID ${n(a.bid)} · Δ ${n(a.ask-a.bid)} · sin clasificar ${n(a.unknown)} · ${a.series} series`:'Sin ventana completa';
  return `<g><title>${esc(`Strike ${r.strike} · 5 min · muestra suscrita · PUT: ${detail(r.putFlow)} · CALL: ${detail(r.callFlow)}`)}</title>${side(r.putFlow,r.y,false)}${side(r.callFlow,r.y,true)}<text x="195" y="${r.y+4}" text-anchor="middle" fill="#fff" font-size="13" font-weight="700">${r.strike.toFixed(2)}</text></g>`;
 }).join('')+(!rows.length?`<text x="195" y="${height/2}" text-anchor="middle" fill="#a6c7e4" font-size="11">Esperando cadena y flujo del horizonte</text>`:'')+`<text x="195" y="${height-70}" text-anchor="middle" fill="#17d6a2" font-size="10">ASK verde · BID rojo · 5 min</text><text x="195" y="${height-52}" text-anchor="middle" fill="#a6c7e4" font-size="10">Δ y sin clasificar: pasa el cursor por el strike</text>`;
}
