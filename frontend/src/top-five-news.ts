import {shortlistNative,nativePriority,nativeReports,type NativeCandidate} from './native-top-five';
import {assetNews,catalystPriority,marketNewsFresh,marketNewsReport} from './market-news';
import {newsCard} from './investment-news';
import './top-five-news.css';
import {escapeHtml as esc} from './model';

export const newsRankingVersion='top-five-news-v1:0.8-native-plus-0.2-max-catalyst;project-heuristic';
export function combinedPriority(c:NativeCandidate,rows:NativeCandidate[],now=Date.now(),reports=nativeReports):{score:number;native:number|null;news:number;fresh:boolean} {
 const native=nativePriority(c,reports[c.symbol],now),fresh=marketNewsFresh(now);
 const news=fresh?Math.max(0,...assetNews(c.symbol,now).map(n=>catalystPriority(n,now))):0;
 const base=native??100*c.volume/Math.max(1,...rows.map(r=>r.volume));
 return {score:fresh ? .8*base+.2*news : base,native,news,fresh};
}
export function rankNewsAssets<T extends NativeCandidate>(rows:T[],now=Date.now(),reports=nativeReports):T[] {
 const pool=shortlistNative(rows),scores=new Map(pool.map(c=>[c.symbol,combinedPriority(c,pool,now,reports)]));
 return pool.sort((a,b)=>{
  const x=scores.get(a.symbol)!,y=scores.get(b.symbol)!;
  return Number(y.native!=null)-Number(x.native!=null)||y.score-x.score||b.volume-a.volume||a.symbol.localeCompare(b.symbol);
 }).slice(0,5);
}
export function assetNewsPanel(c:NativeCandidate,pool:NativeCandidate[],now=Date.now(),enabled=true):string {
 const score=combinedPriority(c,pool,now);
 const meta=enabled&&score.fresh?`Prioridad combinada ${score.score.toFixed(0)}/100 · ${score.native==null?'base por volumen; opciones pendientes':'opciones verificadas'} · catalizador ${score.news.toFixed(0)}/100`:'Noticias sin peso en el ranking: '+(!enabled?'consulta de otra sesión':marketNewsReport()?'canal pendiente de actualizar':'esperando el canal');
 return catalystPanel(c.symbol,now,enabled,meta);
}
export function botAssetNewsPanel(symbol:string,now=Date.now()):string {
 return catalystPanel(symbol,now,true,marketNewsFresh(now)?'Contexto de noticias para este activo del bot':'Contexto pendiente de actualizar');
}
function catalystPanel(symbol:string,now:number,enabled:boolean,meta:string):string {
 const items=enabled?assetNews(symbol,now):[],fresh=marketNewsFresh(now),coverage=marketNewsReport()?.truncated;
 const positive=items.filter(n=>n.sentiment==='positive'||n.eventType==='upgrade').length;
 const negative=items.filter(n=>n.sentiment==='negative'||n.eventType==='downgrade').length;
 const reading=positive&&negative?'Catalizadores contrapuestos':positive?'Contexto favorable':negative?'Presión negativa':items.length?'Sin dirección verificable':'Sin catalizador identificado';
 return `<section class="top-five-news ir-news-channel"><h3>NOTICIAS · INTRADÍA</h3><p class="top-five-news-reading">${reading} · ${items.length}/5 noticias</p><p class="top-five-news-meta">${meta}${coverage?' · cobertura parcial':''}</p>${items.length?`<p class="top-five-news-lead">${esc(items[0].title.replace(/<[^>]*>/g,' '))}</p><details><summary>Ver ${items.length} noticias en español${!fresh?' · última consulta':''}</summary><div class="top-five-news-list">${items.map(newsCard).join('')}</div></details>`:'<p class="top-five-news-meta">Sin noticias relevantes traducidas y vinculadas a este símbolo en las últimas 24 horas.</p>'}<small>El sentimiento describe la noticia; el movimiento observado puede diferir.</small></section>`;
}
export const topFiveNewsMethod='Con noticias vigentes: 80% prioridad nativa y 20% catalizador principal, por importancia y antigüedad (últimas 24 horas). Sin opciones verificadas, la base usa volumen relativo y permanece detrás de los activos verificados. Hasta cinco noticias por activo; sin canal vigente se conserva el ranking original. La lectura negativa también identifica actividad intradía; el índice no indica dirección ni probabilidad de ganancia.';

