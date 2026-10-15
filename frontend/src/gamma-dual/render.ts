import {escapeHtml as esc,price} from '../model';
import type {DualData} from './model';
export interface View {range?:[number,number];visible:number;offset:number;expanded:boolean;vwap:boolean}
export const defaultView=():View=>({visible:120,offset:0,expanded:false,vwap:true});
const money=(n:number)=>Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:1}).format(n);
export function renderDual(data:DualData,view:View=defaultView()):string {
 const rows=data.rows.filter(r=>Number.isFinite(r.strike)&&r.strike>0&&Number.isFinite(r.netGex)&&data.spot!=null&&Math.abs(r.strike/data.spot-1)<=.05);
 const end=Math.max(0,data.bars.length-view.offset),bars=data.bars.slice(Math.max(0,end-view.visible),end);
 const values=[...bars.map(b=>b.close),...rows.map(r=>r.strike),...data.levels.map(l=>l.price)].filter(v=>Number.isFinite(v)&&v>0);
 if(!values.length)return `<section class="gd-root"><p class="gd-empty">${esc(data.symbol)} · Esperando precio y exposición reales.</p></section>`;
 const low=Math.min(...values),high=Math.max(...values),pad=Math.max((high-low)*.06,high*.001);
 const range=view.range??[low-pad,high+pad],y=(p:number)=>45+(range[1]-p)/(range[1]-range[0])*470;
 const peak=Math.max(1,...rows.map(r=>Math.abs(r.netGex)));
 const session=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const panel=(second:boolean)=>{
  const id=second?'detail':'net',zero=490,radius=225,priceEnd=260,right=870;
  const x=(i:number)=>55+(i/Math.max(1,bars.length-1))*(priceEnd-55);
  const axis=[-1,-.5,0,.5,1].map(t=>`<line x1="${zero+t*radius}" x2="${zero+t*radius}" y1="45" y2="515" class="${t===0?'gd-zero':'gd-grid'}"/><text x="${zero+t*radius}" y="25" text-anchor="middle" class="gd-axis">${money(t*peak)}</text>`).join('');
  const priceAxis=Array.from({length:9},(_,i)=>{const p=range[0]+(range[1]-range[0])*i/8;return `<line x1="55" x2="${right}" y1="${y(p)}" y2="${y(p)}" class="gd-grid"/><text x="47" y="${y(p)+3}" text-anchor="end" class="gd-axis">${price(p)}</text>`;}).join('');
  const lines=(vwap=false)=>{
   let segment:string[]=[],html='';const flush=()=>{if(segment.length>1)html+=`<polyline points="${segment.join(' ')}" class="${vwap?'gd-vwap':'gd-price'}"/>`;segment=[];};
   bars.forEach((b,i)=>{const prior=bars[i-1],value=vwap?b.vwap:b.close;if(value==null||!Number.isFinite(value)||prior&&(session(b.timestampMs)!==session(prior.timestampMs)||b.timestampMs-prior.timestampMs>data.interval*60000))flush();if(value!=null&&Number.isFinite(value))segment.push(`${x(i)},${y(value)}`);});flush();return html;
  };
  const sorted=rows.slice().sort((a,b)=>a.strike-b.strike),gaps=sorted.slice(1).map((r,i)=>Math.abs(y(r.strike)-y(sorted[i].strike))).filter(g=>g>0),thickness=Math.max(1,Math.min(3,gaps.length?Math.min(...gaps)*.45:3));
  const profile=rows.map(r=>{const size=Math.abs(r.netGex)/peak*radius,color=second?(r.netGex>=0?'#22dce8':'#ba51e6'):(r.netGex>=0?'#26f46b':'#ff4958');return `<g class="gd-strike"><title>Strike ${price(r.strike)} · GEX ${money(r.netGex)} USD / 1%</title><rect x="${r.netGex>=0?zero:zero-size}" y="${y(r.strike)-thickness/2}" width="${size}" height="${thickness}" fill="${color}"/></g>`;}).join('');
  const levels=data.levels.map(l=>`<line x1="55" x2="${right}" y1="${y(l.price)}" y2="${y(l.price)}" stroke="${l.color}" stroke-width="1.5" stroke-dasharray="4 5"/><text x="${right-5}" y="${y(l.price)-5}" text-anchor="end" fill="${l.color}" class="gd-axis">${esc(l.label)} ${price(l.price)}</text>`).join('');
  const pointMax=Math.max(1,...(data.points?.points??[]).map(p=>Math.abs(p.value)));
  const points=second?(data.points?.points??[]).filter(p=>p.strike>=range[0]&&p.strike<=range[1]).map(p=>`<circle cx="${790+p.value/pointMax*65}" cy="${y(p.strike)}" r="1.7" fill="${p.side==='buy'?'#83ef32':'#ff454f'}"><title>${esc(data.points!.label)} · ${price(p.strike)} · ${p.value} ${esc(data.points!.unit)}</title></circle>`).join(''):'';
  const times=bars.filter((_,i)=>i%Math.max(1,Math.ceil(bars.length/4))===0).map(b=>{const i=bars.indexOf(b);return `<text x="${x(i)}" y="545" text-anchor="middle" class="gd-time">${esc(new Date(b.timestampMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'}))}</text>`;}).join('');
  return `<article class="gd-panel ${second?'gd-detail':''}"><header><strong>${esc(data.symbol)} · ${second?'PERFIL GEX · CIAN / VIOLETA':'GAMMA · GEX NETO'}</strong><small>USD / movimiento del 1 %</small></header><svg data-gd-chart viewBox="0 0 930 580" role="img" aria-label="${second?'Segunda gráfica GEX':'Mapa gamma neto'}"><defs><clipPath id="gd-${id}-clip"><rect x="55" y="45" width="815" height="470"/></clipPath></defs>${axis}${priceAxis}<g clip-path="url(#gd-${id}-clip)">${profile}${levels}${lines()}${view.vwap?lines(true):''}${points}<line x1="${priceEnd}" x2="${priceEnd}" y1="45" y2="515" stroke="#22dce866" stroke-dasharray="2 4"/></g>${times}<text x="490" y="562" text-anchor="middle" class="gd-axis">− GEX ← 0 → + GEX</text></svg><footer>${second?(data.points?esc(data.points.label+' · '+data.points.unit):'Puntos: fuente y métrica pendientes'):'Verde positivo · rojo negativo'} · ${rows.length} strikes${data.partial?' · cobertura parcial':''}</footer></article>`;
 };
 return `<section class="gd-root ${view.expanded?'gd-expanded':''}"><div class="gd-tools"><strong>${esc(data.symbol)} · DOS GRÁFICAS</strong><div>${[1,2,5,15].map(n=>`<button data-gd-interval="${n}" aria-pressed="${data.interval===n}">${n}m</button>`).join('')}</div>${[['older','←'],['newer','→'],['zoom-in','+'],['zoom-out','−'],['reset','Restablecer'],['expand',view.expanded?'Reducir':'Ampliar'],['vwap',view.vwap?'VWAP ✓':'VWAP']].map(([action,label])=>`<button data-gd-action="${action}">${label}</button>`).join('')}</div><p class="gd-status">${esc(data.status)} · ${data.observedMs?esc(new Date(data.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))+' ET':'Consulta pendiente'}</p><div class="gd-panels">${panel(false)}${panel(true)}</div><p class="gd-note">Precio cian · perfiles de snapshot, no gamma histórica · rango ±5 % · ambas gráficas comparten escala de precios. La segunda usa GEX neto cian/violeta hasta definir la métrica de referencia. No se generan puntos sin datos explícitos.</p></section>`;
}
