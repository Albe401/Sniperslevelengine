import {it,expect,afterEach} from 'vitest';
import {publishMarketNews,assetNews,newsSymbols,catalystPriority} from './market-news';
import {rankNewsAssets,combinedPriority,assetNewsPanel} from './top-five-news';
import type {RadarNews} from './investment-news';
import type {EtfReport} from './etf-dealers';
const now=Date.parse('2026-10-09T15:00:00Z');
const story=(id:string,ticker='ON',overrides:Partial<RadarNews>={}):RadarNews=>({id,ticker,title:'Resultados superiores a lo esperado',summary:'Catalizador verificado',source:'Benzinga',publishedAt:new Date(now-60000).toISOString(),horizon:'intraday',sentiment:'positive',importance:'high',translated:true,...overrides});
const candidate=(symbol:string,volume=100)=>({symbol,volume,price:100,changePct:1,volumeMetrics:{rvolPremarket:2}});
const report=(symbol:string):EtfReport=>({symbol,days:7,at:now,spot:100,priceAt:now,contracts:2,expirations:['2026-10-09'],gammaComplete:true,deltaComplete:true,rows:[{strike:100,call:{oi:100,volume:100,delta:5000,gamma:25000},put:{oi:100,volume:50,delta:4000,gamma:25000}}],enrichment:{priceFresh:true,oiKnown:2,gammaKnown:2,deltaKnown:2,quotesFresh:2,details:[],oiReferenceDate:null,previousObservedDate:null,oiComparable:false,storageError:null,model:'native'}});
afterEach(()=>publishMarketNews({items:[],fetchedMs:now,truncated:false}));
it('links exact tickers, not substrings or general market headlines',()=>{
 publishMarketNews({items:[story('1','SONY'),story('2','ON · NVDA'),story('3','Mercado')],fetchedMs:now,truncated:false});
 expect(assetNews('ON',now).map(n=>n.id)).toEqual(['2']);
 expect(newsSymbols(story('4','SONY',{tickerSymbols:['ON','NVDA']}))).toEqual(['ON','NVDA']);
});
it('deduplicates and caps each asset at five relevant translated stories',()=>{
 const stories=Array.from({length:8},(_,i)=>story(String(i)));
 publishMarketNews({items:[...stories,stories[0],story('low','ON',{importance:'low'}),story('english','ON',{translated:false}),story('future','ON',{publishedAt:new Date(now+1).toISOString()}),story('old','ON',{publishedAt:new Date(now-86400001).toISOString()})],fetchedMs:now,truncated:true});
 expect(assetNews('ON',now)).toHaveLength(5);
 expect(new Set(assetNews('ON',now).map(n=>n.id)).size).toBe(5);
 expect(assetNewsPanel(candidate('ON'),[candidate('ON')],now)).toContain('cobertura parcial');
});
it('uses positive and negative material news equally as activity catalysts',()=>{
 expect(catalystPriority(story('1'),now)).toBe(catalystPriority(story('2','ON',{sentiment:'negative'}),now));
 expect(catalystPriority(story('3','ON',{sentiment:'unknown'}),now)).toBeLessThan(catalystPriority(story('1'),now));
});
it('promotes a candidate beyond the original five through news without overriding verified options',()=>{
 const rows=Array.from({length:8},(_,i)=>candidate('A'+i,100-i));
 publishMarketNews({items:[story('1','A7',{eventType:'upgrade'})],fetchedMs:now,truncated:false});
 expect(rankNewsAssets(rows,now,{})[0].symbol).toBe('A7');
 expect(rankNewsAssets(rows,now,{A0:report('A0')})[0].symbol).toBe('A0');
 expect(rankNewsAssets(rows,now,{})).toHaveLength(5);
 expect(combinedPriority(rows[7],rows,now,{}).news).toBeGreaterThan(99);
});
it('falls back to the original order when news is stale and excludes future reports',()=>{
 const rows=[candidate('A',100),candidate('B',99)];
 publishMarketNews({items:[story('1','B')],fetchedMs:now,truncated:false});
 expect(rankNewsAssets(rows,now+180001,{})[0].symbol).toBe('A');
 expect(combinedPriority(rows[1],rows,now-1,{}).news).toBe(0);
});
it('escapes the news display and disables historical session news weighting',()=>{
 publishMarketNews({items:[story('1','ON',{title:'<script>bad</script>',summary:'<img src=x onerror=bad>',eventType:'downgrade',sentiment:'negative'})],fetchedMs:now,truncated:false});
 const html=assetNewsPanel(candidate('ON'),[candidate('ON')],now);
 expect(html).toContain('analyst-change');expect(html).toContain('Presión negativa');expect(html).not.toContain('<script>');expect(html).not.toContain('<img');
 const old=assetNewsPanel(candidate('ON'),[candidate('ON')],now,false);
 expect(old).toContain('consulta de otra sesión');expect(old).not.toContain('Catalizador verificado');
});
