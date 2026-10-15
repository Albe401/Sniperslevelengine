import type {RadarNews} from './investment-news';
export interface MarketNewsReport {items:RadarNews[];fetchedMs:number;truncated:boolean;coverage?:import('./news-pipeline').NewsCoverage}
let report:MarketNewsReport|undefined;
let revision=0;
export const marketNewsRevision=()=>revision;
export const marketNewsReport=()=>report;
export function publishMarketNews(next:MarketNewsReport):void {
 if(!Number.isFinite(next.fetchedMs)||next.fetchedMs<=0||report&&next.fetchedMs<report.fetchedMs)return;
 report=next;revision++;
 if(typeof document!=='undefined')document.dispatchEvent(new Event('snipers:market-news'));
}
export function marketNewsFresh(now:number):boolean {
 return !!report&&report.coverage?.global.status!=='error'&&report.fetchedMs<=now&&now-report.fetchedMs<=180000;
}
export function newsSymbols(item:RadarNews):string[] {
 return (item.tickerSymbols??item.ticker.split(/[·,\s]+/)).map(s=>s.trim().toUpperCase()).filter(s=>/^[A-Z0-9.-]{1,12}$/.test(s)&&s!=='MERCADO');
}
/** Importance and recency measure a catalyst's relevance, never the likelihood of a price rise. */
export const catalystMethodVersion='news-priority-v1:importance-times-direction-times-linear-24h;project-heuristic';
export function catalystPriority(item:RadarNews,now:number):number {
 const at=Date.parse(item.publishedAt);
 if(!item.translated||item.horizon!=='intraday'||!Number.isFinite(at)||at>now||now-at>86400000)return 0;
 const analyst=item.eventType==='upgrade'||item.eventType==='downgrade';
 if(item.importance==='low'&&!analyst)return 0;
 const importance=analyst||item.importance==='high'?100:60;
 const direction=analyst||['positive','negative','mixed'].includes(item.contentDirection??item.sentiment)?1:.35;
 return importance*direction*(1-.5*(now-at)/86400000);
}
export function assetNews(symbol:string,now=Date.now(),source=report):RadarNews[] {
 const unique=new Map<string,RadarNews>();
 for(const n of source?.items??[])if(n.id&&newsSymbols(n).includes(symbol.toUpperCase())&&catalystPriority(n,now)>0)unique.set(n.id,n);
 const gray=(n:RadarNews)=>Number(['unknown','neutral'].includes(n.sentiment)&&n.eventType!=='upgrade'&&n.eventType!=='downgrade');
 return [...unique.values()].sort((a,b)=>gray(a)-gray(b)||catalystPriority(b,now)-catalystPriority(a,now)||Date.parse(b.publishedAt)-Date.parse(a.publishedAt)||a.id.localeCompare(b.id)).slice(0,5);
}


