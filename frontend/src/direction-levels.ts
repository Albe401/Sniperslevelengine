import {qqqConfirmation} from './qqq-confirmation';
import {readMarketEvidence} from './market-evidence';
import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
import {mapRouteLevels} from './route-level-context';
import {escapeHtml as esc,price} from './model';
export interface DirectionLevels {vanna?:import('./vanna-engine').VannaEngine['levels'];summary:string;detail:string;rows:{price:number;text:string}[]}
export function directionLevels(s:LiveState|undefined,d:DirectionScore,symbol:string,active:boolean,now=Date.now()):DirectionLevels {
 const pending=(detail:string):DirectionLevels=>({summary:'SESGO + NIVELES · PENDIENTE',detail,rows:[]});
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),tick=s?.ticks[symbol];
 if(!active||s?.symbol!==symbol||s.stocks.phase!=='streaming'||!tick||tick.symbol!==symbol||typeof tick.price!=='number'||!Number.isFinite(tick.price)||tick.price<=0||typeof tick.tradeMs!=='number'||!Number.isFinite(tick.tradeMs)||tick.tradeMs>now||now-tick.tradeMs>15000)return pending('Esperando precio vigente del activo.');
 if(d.call==null||d.put==null||!['CALL','PUT'].includes(d.side))return pending('Sin sesgo CALL/PUT direccional; los niveles conservan su contexto.');
 const levels=mapRouteLevels(s,symbol,active,now).filter(l=>l.expiration===today&&l.role!=='vacuum-edge');
 const cw=levels.find(l=>l.role==='call-wall'),pw=levels.find(l=>l.role==='put-wall');
 if(!cw||!pw||pw.price>=cw.price)return pending('Paredes del día ausentes o corredor inválido.');
 const sign=d.side==='CALL'?1:-1,spot=tick.price;
 const ordered=[...new Map(levels.map(l=>[l.price,l])).values()].sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot));
 const next=ordered.find(l=>(l.price-spot)*sign>spot*.0003);
 const backing=sign===1?pw:cw,hasBacking=(spot-backing.price)*sign>=0;
 const near=ordered.find(l=>Math.abs(l.price-spot)<=spot*.0003);
 const common=readMarketEvidence(s,symbol,active,now);
 const pressure=common.score.call!=null?(common.support==='aligned'?'CVD reciente acompaña':common.support==='opposed'?'CVD reciente contrario':'CVD reciente pendiente'):d.cvd?.status==='confirms'?'CVD acompaña':d.cvd?.status==='contradicts'?'CVD contrario':'CVD pendiente';
 const basket=qqqConfirmation(s,symbol,active,d.side,now);
 const rows=ordered.map(l=>({price:l.price,text:`${Math.abs(l.price-spot)<=spot*.0003?'PRECIO CERCA':l.price===backing.price?(hasBacking?'RESPALDO POTENCIAL':'RESPALDO REBASADO'):l.price===next?.price?'PRÓXIMA REFERENCIA':(l.price-spot)*sign>0?'EN EL RECORRIDO':'DETRÁS DEL PRECIO'} · ${pressure}${l.provisional?' · provisional':''}${basket?' · '+basket.label:''}`}));
 return {summary:`${d.side} · ${pressure}${basket?' · '+basket.label:''}`,detail:`${near?`Precio cerca de ${near.name} ${price(near.price)}. `:''}${hasBacking?`Respaldo potencial: ${backing.name} ${price(backing.price)}.`:`Precio más allá de ${backing.name}; respaldo rebasado.`} ${next?`Próxima referencia: ${next.name} ${price(next.price)}.`:'Sin referencia delante en el mapa disponible.'} Posición y presión; no acredita defensa, ruptura ni absorción sin reacción persistente.${basket?' '+basket.detail:''}`,rows};
}
export function directionLevelsPanel(read:DirectionLevels):string {
 return `<section class="direction-level-link" aria-label="Confluencia y CVD en los niveles"><strong>${esc(read.summary)}</strong><details data-horizon-details="direction-level-link"><summary>Sesgo y recorrido</summary><p>${esc(read.detail)}</p></details></section>`;
}
export function directionLevelTag(read:DirectionLevels|undefined,strike:number):string {
 const row=read?.rows.find(r=>r.price===strike);
 return row?`<small class="direction-level-tag">${esc(row.text)}</small>`:'';
}
