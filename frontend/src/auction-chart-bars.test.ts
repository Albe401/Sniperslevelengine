import {describe,it,expect} from 'vitest';
import {auctionChartBarsSvg,chartAuctionProfile} from './auction-chart-bars';
import type {AuctionReading} from './auction-map-panel';
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {profileGroups} from './auction-volume-profile';
const now=Date.UTC(2026,9,13,15);
const fixture=():AuctionReading=>({symbol:'QQQ',version:'auction-map-ticks-v1',asOfMs:now,provisional:true,acceptedTrades:3,totalVolume:150,pocLow:101,pocHigh:102,val:100,vah:102,calibrated:false,reaction:'pending',routes:[],bins:[{low:100,high:101,volume:50,brackets:2,volumeRatio:.5,timeRatio:.7,kind:'acceptance'},{low:101,high:102,volume:100,brackets:2,volumeRatio:1,timeRatio:1,kind:'acceptance'}]});
describe('AMT bars inside price chart',()=>{
 it('aligns to candle price coordinates and begins exactly at the left edge',()=>{const html=auctionChartBarsSvg(chartAuctionProfile('QQQ',true,now,fixture()),p=>(103-p)*100,800,400,now);expect(html).toContain('x="0" y="200.000" width="70.000"');expect(html).toContain('x="0" y="100.000" width="140.000"');expect(html).toContain('data-amt-bar="poc"');expect(html).toContain('#ac8be7');expect(html).toContain('#20bb89');expect(html).not.toContain('<text');});
 it('clips outside prices and follows scale changes without imposing a price range',()=>{const v=chartAuctionProfile('QQQ',true,now,fixture());expect(auctionChartBarsSvg(v,()=>-20,800,300,now)).toBe('');expect(auctionChartBarsSvg(v,()=>null,800,300,now)).toBe('');expect(auctionChartBarsSvg(v,p=>(102-p)*40,300,80,now)).toContain('width="54.000"');});
 it('does not draw future, foreign, invalid or OHLC-estimated profiles',()=>{for(const r of [{...fixture(),symbol:'SPY'},{...fixture(),asOfMs:now+1},{...fixture(),version:'auction-map-bars-reference-v1'},{...fixture(),totalVolume:NaN}])expect(chartAuctionProfile('QQQ',true,now,r)).toBeUndefined();});
 it('retains observed same-day bars without presenting old rejection as current',()=>{const r=fixture();const v=chartAuctionProfile('QQQ',false,now+20000,r);expect(v?.state).toBe('retained');expect(auctionChartBarsSvg(v,p=>(103-p)*100,800,400,now+20000)).toContain('fill-opacity="0.22"');expect(chartAuctionProfile('QQQ',false,now+86400000,r)).toBeUndefined();});
 it('does not equate low participation with rejection',()=>{const r=fixture();r.bins[0].kind='low-participation';const html=auctionChartBarsSvg(chartAuctionProfile('QQQ',true,now,r),p=>(103-p)*100,800,400,now);expect(html).toContain('data-amt-bar="volume"');expect(html).not.toContain('data-amt-bar="rejection"');});
 it('colors rejection only when the supplied measured evidence is current',()=>{const r=fixture();r.rejectionUpper={version:'auction-rejection-index-v1',symbol:'QQQ',status:'observed',score:100,distance:1,permanence:1,participation:1,level:100,windowStartMs:now-300000,windowEndMs:now,calibrated:false,probability:null};const html=auctionChartBarsSvg(chartAuctionProfile('QQQ',true,now,r),p=>(103-p)*100,800,400,now);expect(html).toContain('data-amt-bar="rejection"');expect(html).toContain('#ff6273');});
 const realPath=new URL('../../../docs/audits/2026-10-13-poc-partitions/native-partitions.json',import.meta.url);
 (existsSync(realPath)?it:it.skip)('verifies grouping conservation and chart geometry with the saved real QQQ profile',()=>{
  const r=JSON.parse(readFileSync(realPath,'utf-8')).engine as AuctionReading;
  const lo=Math.min(...r.bins.map(b=>b.low)),hi=Math.max(...r.bins.map(b=>b.high)),height=550,width=1000,coordinate=(p:number)=>height*(hi-p)/(hi-lo);
  const groups=profileGroups(r.bins,180),sum=groups.reduce((a,b)=>a+b.volume,0),error=Math.abs(sum-r.totalVolume);
  expect(error).toBeLessThan(1e-6);
  const view=chartAuctionProfile('QQQ',true,r.asOfMs,r)!;expect(view.state).toBe('live');const html=auctionChartBarsSvg(view,coordinate,width,height,r.asOfMs);
  expect(html).toContain('data-amt-bar="poc"');expect(html).not.toContain('<text');
  for(const match of html.matchAll(/x="([^"]+)" y="([^"]+)" width="([^"]+)" height="([^"]+)"/g)){expect(Number(match[1])).toBe(0);expect(Number(match[2])).toBeGreaterThanOrEqual(0);expect(Number(match[2])).toBeLessThanOrEqual(height);expect(Number(match[3])).toBeLessThanOrEqual(140);}
  const directory=new URL('../../../docs/audits/2026-10-13-amt-chart-bars/',import.meta.url);
  writeFileSync(new URL('real-profile-bars.svg',directory),`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" style="background:#091327">${html}</svg>`);
  writeFileSync(new URL('real-render-verification.json',directory),JSON.stringify({source:String(realPath),symbol:r.symbol,asOfMs:r.asOfMs,unit:'shares',sourceTotal:r.totalVolume,groupedTotal:sum,absoluteVolumeDifference:error,groups:groups.length,priceRange:[lo,hi],width,height,barMaximumWidth:140,formula:'width=min(140,0.18*chartWidth)*groupVolume/maxGroupVolume; y=series.priceToCoordinate(price)',labelsOrPanelsAdded:false,ordersSent:0,scope:'Saved real-profile renderer check; no live native UI certification'},null,2));
 });
});
