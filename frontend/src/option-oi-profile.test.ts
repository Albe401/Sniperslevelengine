import {it,expect} from 'vitest';
import {readFileSync} from 'node:fs';
import {buildOiHeatmap,type Contract} from './option-oi-heatmap';
import {oiStrikeRows,reportOiProfile} from './option-oi-profile';
import type {EtfReport} from './etf-dealers';
const c=(side='call',expiration='2026-10-16',shares=100):Contract=>({ticker:`O:QQQ${expiration.slice(2).replaceAll('-','')}${side==='call'?'C':'P'}00750000`,symbol:'QQQ',expiration,strike:750,side,sharesPerContract:shares,extraDeliverables:0});
const a=c(),b=c('call','2026-10-19'),p=c('put');
const input={symbol:'QQQ',observedMs:1791999987968,oiDate:null,populationId:'projection-tests',referenceComplete:true,expected:[a,b,p],observations:[{...a,oi:10},{...b,oi:20},{...p,oi:0}]};
it('sums expirations by side and preserves real zero',()=>expect(oiStrikeRows(buildOiHeatmap(input))).toEqual([{strike:750,call:30,put:0}]));
it('missing expiry invalidates its total without contaminating the other side',()=>expect(oiStrikeRows(buildOiHeatmap({...input,observations:input.observations.slice(1)}))).toEqual([{strike:750,call:null,put:0}]));
it('adjusted contracts are excluded and unknown deliverables remain unknown',()=>{
 expect(oiStrikeRows(buildOiHeatmap({...input,expected:[c('call','2026-10-16',50),p]}))[0]).toEqual({strike:750,call:0,put:0});
 expect(oiStrikeRows(buildOiHeatmap({...input,expected:[{...a,sharesPerContract:null},p]}))[0].call).toBeNull();
});
it('duplicates do not add and conflicts cannot become totals',()=>{
 expect(oiStrikeRows(buildOiHeatmap({...input,observations:[...input.observations,input.observations[0]]}))[0].call).toBe(30);
 expect(oiStrikeRows(buildOiHeatmap({...input,observations:[...input.observations,{...a,oi:99}]}))[0].call).toBeNull();
});
it('reconciles every real strike independently across all 23 expirations and a single expiry',()=>{
 const raw=JSON.parse(readFileSync(new URL('../../../docs/audits/2026-10-14-oi-heatmap/integration-v5/real-live-report.json',import.meta.url),'utf8')) as EtfReport;
 for(const dates of [raw.expirations,[raw.expirations[0]]]){
  const report={...raw,expirations:dates};const {map,rows}=reportOiProfile(report);
  const refs=new Map((raw.heatmap!.reference as any[]).filter(r=>dates.includes(r.expiration_date)&&r.shares_per_contract===100&&(!r.additional_underlyings||r.additional_underlyings.length===0)).map(r=>[r.ticker,r]));
  const sums=new Map<string,number>();for(const s of raw.heatmap!.snapshots as any[]){const r=refs.get(s.details.ticker);if(!r)continue;const key=JSON.stringify([r.strike_price,r.contract_type]);sums.set(key,(sums.get(key)??0)+s.open_interest);}
  for(const row of rows)for(const side of ['call','put'] as const)expect(row[side]).toBe(sums.get(JSON.stringify([row.strike,side]))??0);
  expect(map.coverage).toBe(1);
  if(dates.length===23){expect(rows.reduce((n,r)=>n+r.call!,0)).toBe(4204318);expect(rows.reduce((n,r)=>n+r.put!,0)).toBe(6420767);}
 }
});
