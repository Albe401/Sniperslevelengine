import {describe,it,expect} from 'vitest';
import {newNews,importantNews,watchedNews,audibleNews} from './news-alerts';
import type {RadarNews} from './investment-news';
const item={id:'1',ticker:'ON',title:'Prueba',summary:'',source:'Benzinga',publishedAt:'',sentiment:'neutral',horizon:'intraday'} as RadarNews;
describe('global news alerts',()=>{
 it('ignores seen stories and duplicates in an arrival batch',()=>{expect(newNews([item,item,{...item,id:'2'}],new Set(['2'])).map(n=>n.id)).toEqual(['1']);});
 it('includes important neutral stories and analyst rating changes',()=>{expect(importantNews({...item,importance:'high'})).toBe(true);expect(importantNews({...item,eventType:'downgrade',importance:'low'})).toBe(true);expect(importantNews({...item,importance:'low'})).toBe(false);});
 it('matches any of the five watched symbols exactly, independent of the active asset',()=>{
  const symbols=['ON','NVDA','AAPL','SPY','QQQ'];
  expect(watchedNews({...item,ticker:'SONY'},symbols)).toBe(false);
  expect(watchedNews({...item,ticker:'NVDA · MSFT'},symbols)).toBe(true);
  expect(watchedNews({...item,ticker:'Mercado'},symbols)).toBe(false);
 });
 it('always includes watched neutral and low-relevance stories but filters other companies',()=>{
  const watched={...item,importance:'low' as const},other={...item,id:'2',ticker:'MSFT',importance:'high' as const};
  expect(audibleNews([watched,other],['ON'],true,true)).toEqual([watched]);
  expect(audibleNews([watched,other],['ON'],false,true)).toEqual([watched,other]);
  expect(audibleNews([watched],['AAPL'],true,false)).toEqual([]);
 });
});

it('raw critical reports pass the important filter before translation',()=>{expect(importantNews({...item,importance:'low',translated:false,rawCritical:true})).toBe(true);});
