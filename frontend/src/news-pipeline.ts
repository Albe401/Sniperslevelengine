import {invoke} from './snapshot-observation';
import type {MarketNewsReport} from './market-news';
import type {RadarNews} from './investment-news';
export interface NewsCoverage {version:string;global:{status:'ok'|'error';truncated:boolean;fetchedMs:number|null};symbols:{symbol:string;status:'global'|'ok'|'error';truncated:boolean;fetchedMs:number|null}[];failures:number;method:string}
export async function queryNews(symbols:string[]):Promise<{report:MarketNewsReport|null;items:RadarNews[];coverage:NewsCoverage}> {
 let global:MarketNewsReport|undefined;try{global=await invoke('get_benzinga_news',{ticker:''});}catch{/* Explicitly preserved below. */}
 const coverage:NewsCoverage={version:'news-coverage-v2',global:{status:global?'ok':'error',truncated:global?.truncated??true,fetchedMs:global?.fetchedMs??null},symbols:[],failures:global?0:1,method:'Global 24h query, targeted queries only when global incomplete or failed; not all-market reception certification'};
 const items=[...(global?.items??[])],times=global?[global.fetchedMs]:[];
 if(global&&!global.truncated){coverage.symbols=symbols.map(symbol=>({symbol,status:'global',truncated:false,fetchedMs:global!.fetchedMs}));}
 else {const results=await Promise.allSettled(symbols.map(ticker=>invoke<MarketNewsReport>('get_benzinga_news',{ticker})));results.forEach((result,i)=>{if(result.status==='fulfilled'){items.push(...result.value.items);times.push(result.value.fetchedMs);coverage.symbols.push({symbol:symbols[i],status:'ok',truncated:result.value.truncated,fetchedMs:result.value.fetchedMs});}else{coverage.failures++;coverage.symbols.push({symbol:symbols[i],status:'error',truncated:true,fetchedMs:null});}});}
 const unique=new Map<string,RadarNews>();for(const n of items){const old=unique.get(n.id);if(!old|| (n.revision??1)>(old.revision??1)||(n.revision??1)===(old.revision??1)&&!!n.translated&&!old.translated)unique.set(n.id,n);}
 const merged=[...unique.values()];
 return {items:merged,coverage,report:times.length?{items:merged,fetchedMs:Math.min(...times),truncated:coverage.global.truncated||coverage.failures>0||coverage.symbols.some(x=>x.truncated),coverage}:null};
}
/** Alert identity uses original content; query metadata and translation are not new news. */
export const newsEventKey=(n:RadarNews)=>(n.originalTitle??n.title)!=null
 ?JSON.stringify([n.source,n.publishedAt.slice(0,10),n.originalTitle??n.title,n.originalSummary??n.summary??''])
 :JSON.stringify([n.eventId??n.id,n.revision??1]);
export const readyNews=(n:RadarNews)=>!!n.translated||n.rawCritical===true;

/** Linear pass over retained IDs and current items; preserves revision semantics. */
export function unseenRevisions(items:RadarNews[],seen:Set<string>,events:Set<string>):RadarNews[]{
 const excluded=new Set(seen);for(const n of items)if((n.revision??1)>1&&!events.has(newsEventKey(n)))excluded.delete(n.id);
 const unique=new Map<string,RadarNews>();for(const n of items)if(n.id&&!excluded.has(n.id))unique.set(n.id,n);return [...unique.values()];
}
