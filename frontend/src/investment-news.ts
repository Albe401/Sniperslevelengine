import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml} from './model';
import './investment-news.css';
import {publishMarketNews} from './market-news';

export type NewsSentiment='positive'|'negative'|'mixed'|'neutral'|'unknown';
export interface RadarNews {
 provider?:string;contentSource?:string;providerArticleId?:number;providerUpdatedAt?:string;eventId?:string;revision?:number;firstSeenAtMs?:number;lastSeenAtMs?:number;receivedAtMs?:number;translatedAtMs?:number;classifiedAtMs?:number;observedDeliveryLatencyMs?:number|null;verificationStatus?:string;processingState?:string;persistenceState?:string;rawCritical?:boolean;critical?:boolean;category?:string;categories?:string[];subtype?:string;contentDirection?:NewsSentiment;marketReaction?:NewsSentiment;conflictState?:string;classificationVersion?:string;materialityBasis?:string;
id:string;ticker:string;tickerSymbols?:string[];title:string;summary:string;source:string;publishedAt:string;sentiment:NewsSentiment;horizon:'intraday'|'long';url?:string;sentimentBasis?:string;importance?:'high'|'medium'|'low';eventType?:'upgrade'|'downgrade'|'other';reason?:string;originalTitle?:string;originalSummary?:string;translated?:boolean;translationEngine?:string}
const labels:Record<NewsSentiment,string>={positive:'Positiva',negative:'Negativa',mixed:'Mixta',neutral:'Neutral',unknown:'Sin sentimiento'};
const esc=(v:string)=>escapeHtml(v);
let designPreview=false;
let report:{items:RadarNews[];fetchedMs:number;truncated:boolean}|null=null;
let showAll=false,eventFilter='all',newsPage=0;
export const NEWS_PAGE_SIZE=40;
const dayFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
const cardFormatter=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',month:'short',day:'numeric',hour:'2-digit',minute:'2-digit'});
const sectionPaint=new WeakMap<Element,string>();
export function newsPageSlice<T>(items:T[],page:number){const pages=Math.max(1,Math.ceil(items.length/NEWS_PAGE_SIZE)),index=Math.max(0,Math.min(pages-1,Math.floor(Number.isFinite(page)?page:0)));return {items:items.slice(index*NEWS_PAGE_SIZE,(index+1)*NEWS_PAGE_SIZE),index,pages,total:items.length};}
let newsWindow:'today'|'24h'='24h',windowInitialized=false;
export function newsToday(items:RadarNews[],now=Date.now()):RadarNews[]{
 const etDay=(ms:number)=>dayFormatter.format(ms);
 const day=etDay(now);return items.filter(n=>{const ms=Date.parse(n.publishedAt);return Number.isFinite(ms)&&ms<=now&&etDay(ms)===day;});
}
let loading=false,loadError='',symbol='';
let timer:ReturnType<typeof setInterval>|undefined;
const plain=(v:string)=>v.replace(/<[^>]*>/g,' ').replace(/&(?:#39|apos);/g,"'").replace(/&quot;/g,'"').replace(/&amp;/g,'&').replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/\s+/g,' ').trim();
// Conservative headline cues; never presented as vendor sentiment or a trade signal.
export function headlineSentiment(title:string):NewsSentiment {
 const text=plain(title).toLowerCase();
 if(/\b(?:not|no|never|unlikely|could|may|might|if)\b/.test(text))return 'unknown';
 const positive=/\b(?:upgrades?|upgraded|beats? (?:estimates|expectations|consensus)|raises? (?:guidance|outlook|forecast)|raised (?:guidance|outlook|forecast)|shares (?:rise|surge|jump)|record (?:revenue|profit))\b/.test(text);
 const negative=/\b(?:downgrades?|downgraded|miss(?:es|ed)? (?:estimates|expectations|consensus)|(?:cuts?|lowers?|reduced) (?:guidance|outlook|forecast)|shares (?:fall|drop|plunge)|bankruptcy|default|fraud)\b/.test(text);
 return positive&&negative?'mixed':positive?'positive':negative?'negative':'unknown';
}
export function applyHeadlineReading(item:RadarNews):RadarNews {
 if(item.translated)return item;
 if(item.sentiment!=='unknown')return item;
 const sentiment=headlineSentiment(item.originalTitle??item.title);
 return {...item,sentiment,sentimentBasis:sentiment==='unknown'?undefined:'Lectura automática del titular'};
}
export function newsCard(n:RadarNews):string {
 const date=Date.parse(n.publishedAt);
 const time=Number.isFinite(date)?cardFormatter.format(date)+' ET':'Fecha no disponible';
 let url='';try{const parsed=new URL(n.url??'');if(parsed.protocol==='https:'||parsed.protocol==='http:')url=parsed.href;}catch{/* Missing source URL. */}
 return `<article class="ir-news-card ${n.sentiment}${n.eventType==='upgrade'||n.eventType==='downgrade'?' analyst-change':''}"><header><strong>${esc(n.ticker)}</strong><span>${labels[n.sentiment]}</span></header><h3>${esc(plain(n.title))}</h3>${n.eventType==='upgrade'||n.eventType==='downgrade'?`<b class="ir-news-event">${n.eventType==='upgrade'?'↑ Mejora de analista':'↓ Rebaja de analista'}</b>`:''}<p>${esc(plain(n.summary))}</p>${n.reason?`<p class="ir-news-reason">${esc(n.reason)}</p>`:''}${n.sentimentBasis?`<small class="ir-news-basis">${esc(n.sentimentBasis)}</small>`:''}<small>${esc(n.category??'EVENTO PENDIENTE')} · ${esc(n.verificationStatus??'REPORTADO')} · ${n.translated?'Traducida':'Original · traducción pendiente'}${n.revision?' · revisión '+n.revision:''}</small>${n.contentDirection?`<small>Contenido: ${esc(labels[n.contentDirection])} · reacción narrada: ${esc(labels[n.marketReaction??'unknown'])}${n.conflictState==='MIXED'?' · conflicto titular/resumen':' '}</small>`:''}<footer><span>${esc(n.provider??'Data')} / ${esc(n.contentSource??n.source)}</span><time>${esc(time)}</time></footer>${n.originalTitle?`<details class="ir-news-original"><summary>Ver original</summary><h4>${esc(plain(n.originalTitle))}</h4><p>${esc(plain(n.originalSummary??''))}</p></details>`:''}${url?`<a href="${esc(url)}" target="_blank" rel="noopener noreferrer">Leer fuente ↗</a>`:''}</article>`;
}
const samples:RadarNews[]=[
 {id:'demo-day-negative',ticker:'ON',title:'La compañía reduce sus perspectivas de demanda',summary:'Ejemplo: presión sobre ventas y márgenes; revisar la recuperación esperada.',source:'Ejemplo ficticio',publishedAt:'',sentiment:'negative',horizon:'intraday'},
 {id:'demo-day-neutral',ticker:'CAT',title:'Anunciada la próxima publicación de resultados',summary:'Ejemplo: evento para seguimiento, sin lectura direccional.',source:'Ejemplo ficticio',publishedAt:'',sentiment:'neutral',horizon:'intraday'},
 {id:'demo-long-positive',ticker:'AVGO',title:'Mejora de previsiones en infraestructura',summary:'Ejemplo: apoya el crecimiento; revisar cuánto refleja ya la valoración.',source:'Ejemplo ficticio',publishedAt:'',sentiment:'positive',horizon:'long'},
 {id:'demo-long-mixed',ticker:'ORCL',title:'Más contratos y mayores necesidades de inversión',summary:'Ejemplo: el crecimiento favorece el negocio, pero exige más financiación.',source:'Ejemplo ficticio',publishedAt:'',sentiment:'mixed',horizon:'long'},
];
export function newsDisplayOrder(a:RadarNews,b:RadarNews):number {
 const gray=(n:RadarNews)=>Number((n.sentiment==='neutral'||n.sentiment==='unknown')&&n.eventType!=='upgrade'&&n.eventType!=='downgrade');
 const relevance=(n:RadarNews)=>n.eventType==='upgrade'||n.eventType==='downgrade'?3:({high:3,medium:2,low:1}[n.importance??'medium']);
 return gray(a)-gray(b)||relevance(b)-relevance(a)||(Date.parse(b.publishedAt)||0)-(Date.parse(a.publishedAt)||0);
}
export function newsSections(items:RadarNews[]=[],demo=false):string {
 const source=demo?samples:newsWindow==='today'?newsToday(items):items;
 const all=source.filter(n=>n.horizon==='intraday');
 const data=all.filter(n=>(showAll||n.importance!=='low')&&(eventFilter==='all'||n.eventType===eventFilter)).sort(newsDisplayOrder);
 const page=newsPageSlice(data,newsPage);newsPage=page.index;
 const pager=page.pages>1?`<div class="ir-news-pages"><button type="button" data-news-page="-1" ${page.index===0?'disabled':''}>Anterior</button><span>Página ${page.index+1} de ${page.pages} · ${page.items.length} de ${page.total} noticias</span><button type="button" data-news-page="1" ${page.index+1===page.pages?'disabled':''}>Siguiente</button></div>`:'';
 const shortlist=(sentiment:NewsSentiment)=>data.filter(n=>n.sentiment===sentiment).slice(0,3);
 const digest=`<div class="ir-news-digest">${(['positive','negative'] as const).map(t=>`<section><h3>${t==='positive'?'Positivas destacadas':'Negativas destacadas'}</h3>${shortlist(t).length?`<ul>${shortlist(t).map(n=>`<li><b>${esc(n.ticker)}</b> · ${esc(plain(n.title))}</li>`).join('')}</ul>`:'<p>Sin noticias clasificadas en este grupo.</p>'}</section>`).join('')}</div><p class="ir-news-selection-note">${data.length} noticias visibles · ${all.filter(n=>n.importance==='low').length} de baja relevancia ${showAll?'incluidas':'ocultas'}. Primero las clasificadas por relevancia; las grises quedan al final. Dentro de cada nivel, las más recientes primero. El color indica la lectura del contenido, no su importancia.</p>`;
 return `${digest}${pager}${demo?'<p class="ir-news-demo" role="status">Vista de diseño · noticias ficticias, sin datos de mercado.</p>':''}${(['intraday'] as const).map(h=>`<section class="ir-news-horizon" aria-label="Noticias ${h==='intraday'?'intradía':'a largo plazo'}"><header><div><span class="ir-eyebrow">${h==='intraday'?'CATALIZADORES DE LA SESIÓN':'NEGOCIO Y TESIS DE INVERSIÓN'}</span><h2>${h==='intraday'?'Intradía':'Largo plazo'}</h2><p>${newsWindow==='today'?'Publicadas hoy · Nueva York':'Noticias recientes · últimas 24 horas'}</p></div><span class="ir-badge">${data.filter(n=>n.horizon===h).length} noticias</span></header><div class="ir-news-grid">${page.items.filter(n=>n.horizon===h).map(newsCard).join('')||'<div class="ir-news-empty"><strong>Sin noticias visibles con estos filtros</strong><p>Prueba Mostrar baja relevancia, todas las noticias o las últimas 24 horas.</p></div>'}</div></section>`).join('')}`;
}
export function newsLanguageNotice(items:RadarNews[]):string {
 return items.some(n=>!n.translated)?' · Se muestran los originales; la traducción al español está pendiente o no disponible.':items.some(n=>n.translationEngine)?' · Traducidas localmente al español':'';
}
export function newsPanel():string {
 if(!windowInitialized){newsWindow="today";windowInitialized=true;}
 return `<section class="ir-news-channel"><header class="ir-news-toolbar"><div><span class="ir-news-connection">Benzinga · Data</span><p>Noticias intradía en español · relevancia y cambios de analistas</p></div><button type="button" data-ir-news-demo aria-pressed="${designPreview}">${designPreview?'Ocultar ejemplos':'Ver ejemplo de diseño'}</button></header><form data-news-form class="ir-news-controls"><label>Símbolo <input name="ticker" placeholder="Todo el mercado · o AAPL" maxlength="12" value="${esc(symbol)}"></label><label>Fecha<select data-news-window><option value="today" ${newsWindow==='today'?'selected':''}>Hoy · Nueva York</option><option value="24h" ${newsWindow==='24h'?'selected':''}>Últimas 24 horas</option></select></label><label>Tipo de noticia<select data-news-event><option value="all">Todas las relevantes</option><option value="upgrade">Mejoras de analistas</option><option value="downgrade">Rebajas de analistas</option></select></label><label class="ir-news-noise"><input type="checkbox" data-news-all> Mostrar baja relevancia</label><button type="submit" ${!isTauri()?'disabled':''}>Consultar noticias</button></form><div data-news-status role="status"></div><div class="ir-news-legend" aria-label="Sentimiento de noticias"><span class="positive">Positiva</span><span class="negative">Negativa</span><span class="mixed">Mixta</span><span class="analyst-change">Upgrade / Downgrade</span><span class="neutral">Neutral / sin datos</span></div><div data-ir-news-sections>${newsSections(report?.items??[],designPreview)}</div></section>`;
}
function renderNews(root:HTMLElement,paintSections=true):void {
 const container=root.querySelector('[data-ir-news-sections]');if(container&&paintSections){const markup=newsSections(report?.items??[],designPreview);if(sectionPaint.get(container)!==markup){container.innerHTML=markup;sectionPaint.set(container,markup);}root.querySelectorAll<HTMLButtonElement>('[data-news-page]').forEach(button=>{button.onclick=()=>{newsPage+=Number(button.dataset.newsPage);renderNews(root);};});}
 const status=root.querySelector('[data-news-status]');if(status)status.textContent=loadError||(loading?'Buscando noticias nuevas en Benzinga…':report?(newsToday(report.items).length+' noticias de hoy · '+report.items.length+' recibidas en 24 h · Última consulta '+new Date(report.fetchedMs).toLocaleTimeString('es',{timeZone:'America/New_York'})+' ET'+(report.truncated?' · Cobertura parcial: últimas 600 noticias.':'')+newsLanguageNotice(report.items)):isTauri()?'Listo para consultar':'Abre la aplicación Windows para recibir noticias reales.');
 const submit=root.querySelector<HTMLButtonElement>('[data-news-form] button');if(submit)submit.disabled=loading||!isTauri();
}
export async function loadNews(root:HTMLElement,force=false):Promise<void>{
 if(loading||!isTauri()||!root.isConnected||designPreview)return;
 loading=true;loadError='';renderNews(root,false);
 try{report=await invoke('get_benzinga_news',{ticker:symbol,force});if(report){report.items=report.items.map(applyHeadlineReading);if(!symbol)publishMarketNews(report);}}catch(e){loadError='No se pudieron actualizar las noticias: '+String(e)+(report?' · Se conserva la última consulta.':'');}finally{loading=false;renderNews(root);}
}
export function bindNews(root:HTMLElement):void {
 const button=root.querySelector<HTMLButtonElement>('[data-ir-news-demo]');
 if(!button)return;
 button.onclick=()=>{newsPage=0;designPreview=!designPreview;button.setAttribute('aria-pressed',String(designPreview));button.textContent=designPreview?'Ocultar ejemplos':'Ver ejemplo de diseño';renderNews(root);if(!designPreview)void loadNews(root);};
 const form=root.querySelector<HTMLFormElement>('[data-news-form]');if(form)form.onsubmit=e=>{e.preventDefault();const next=String(new FormData(form).get('ticker')??'').trim().toUpperCase();if(next&&!/^[A-Z0-9.-]{1,12}$/.test(next)){loadError='Escribe un solo símbolo válido';renderNews(root);return;}if(loading)return;if(next!==symbol){report=null;newsPage=0;}symbol=next;designPreview=false;button.setAttribute('aria-pressed','false');button.textContent='Ver ejemplo de diseño';void loadNews(root,true);};
 const dateFilter=root.querySelector<HTMLSelectElement>('[data-news-window]');if(dateFilter)dateFilter.onchange=()=>{newsPage=0;newsWindow=dateFilter.value==='today'?'today':'24h';renderNews(root);};
 const noise=root.querySelector<HTMLInputElement>('[data-news-all]');if(noise){noise.checked=showAll;noise.onchange=()=>{newsPage=0;showAll=noise.checked;renderNews(root);};}
 const filter=root.querySelector<HTMLSelectElement>('[data-news-event]');if(filter){filter.value=eventFilter;filter.onchange=()=>{newsPage=0;eventFilter=filter.value;renderNews(root);};}
 renderNews(root);void loadNews(root);
 if(timer)clearInterval(timer);timer=setInterval(()=>{if(!root.isConnected){clearInterval(timer);timer=undefined;return;}if(document.querySelector('.investment-radar[data-ir-active="news"]'))void loadNews(root);},60_000);
}


