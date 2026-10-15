import {escapeHtml as esc,price} from './model';
export interface ContractRanking {covered:number;total:number;calls:RankedContract[];puts:RankedContract[]}
interface RankedContract {ticker:string;expiration:string;strike:number;score:number;oi:number;volume:number}
export function topStrikes(ranking:ContractRanking|undefined,fresh:boolean):string {
 const r=fresh?ranking:undefined;
 const count=(n:number)=>Intl.NumberFormat('es',{maximumFractionDigits:0}).format(n);
 return `<section class="qd-rankings" aria-label="Top cinco strikes CALL y PUT">${(['CALL','PUT'] as const).map(side=>{
  const rows=(side==='CALL'?r?.calls:r?.puts)?.slice(0,5)??[];
  return `<article class="qd-panel qd-ranking ${side==='CALL'?'green':'red'}"><h3>TOP 5 STRIKES ${side}</h3><table><thead><tr><th>STRIKE</th><th>SCORE</th><th>OI</th><th>VOL</th></tr></thead><tbody>${rows.map((c,i)=>`<tr><td><span class="qd-rank">${i+1}.</span> ${price(c.strike)}<small>${esc(c.expiration)}</small></td><td>${c.score.toFixed(1)}</td><td>${count(c.oi)}</td><td>${count(c.volume)}</td></tr>`).join('')||'<tr><td colspan="4" class="qd-ranking-empty">Esperando contratos con datos suficientes</td></tr>'}</tbody></table><p>${r?`${r.covered<r.total?'Provisional · ':''}${r.covered}/${r.total} contratos con cobertura`:'Sin ranking vigente'} · Score de concentración, no probabilidad.</p></article>`;
 }).join('')}</section>`;
}
