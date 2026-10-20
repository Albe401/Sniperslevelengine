import {it,expect} from 'vitest';
import {inventoryEnd,inventoryDates,inventoryTotals,validInventory,inventoryPanel,seedCapturedInventory,inventoryProfile} from './contract-inventory';
import type {EtfReport} from './etf-dealers';
import type {EtfDetail} from './etf-enrichment';
import {gammaRouteChart} from './gamma-live';
import {contractChainPanel} from './investment-contract-chain';
const now=Date.UTC(2026,9,8,15);
const c=(expiration:string,kind='call',oi:number|null=10)=>({ticker:expiration+kind,expiration,kind,strike:100,oi,volume:3,dex:null,gex:null}) as EtfDetail;
function report(){const details=[c('2026-10-08'),c('2026-10-08','put',20),c('2026-10-15'),c('2026-11-05'),c('2026-11-08'),c('2027-04-08')];return {symbol:'QQQ',days:183,at:now,contracts:details.length,enrichment:{details}} as EtfReport;}
it('reads saved inventory at capture time without changing its date or accepting foreign/future evidence',()=>{
 const r=report();seedCapturedInventory(r,'QQQ',now);expect(inventoryProfile('QQQ',now)?.at).toBe(r.at);
 expect(inventoryProfile('QQQ',now+300001)).toBeUndefined();
 seedCapturedInventory({...r,symbol:'SPY'},'QQQ',now);expect(inventoryProfile('QQQ',now)).toBeUndefined();
 seedCapturedInventory({...r,at:now+1},'QQQ',now);expect(inventoryProfile('QQQ',now)).toBeUndefined();
 seedCapturedInventory(undefined,'QQQ',now);
});
it.each([['2026-10-08','2027-04-08'],['2026-08-31','2027-02-28'],['2027-08-31','2028-02-29'],['2026-12-31','2027-06-30']])('uses six calendar months from %s',(start,end)=>expect(inventoryEnd(start)).toBe(end));
it('distinguishes four weeks, one calendar month and six months',()=>{const r=report();expect(inventoryDates(r,'DAY',now)).toEqual(['2026-10-08']);expect(inventoryDates(r,'WEEK',now)).toHaveLength(2);expect(inventoryDates(r,'FOUR_WEEKS',now)).toHaveLength(3);expect(inventoryDates(r,'MONTH',now)).toHaveLength(4);expect(inventoryDates(r,'SIX_MONTHS',now)).toHaveLength(5);});
it('bounds the three-month OI view by calendar date',()=>{const r=report();r.enrichment!.details.push(c('2027-01-08'),c('2027-01-09'));expect(inventoryDates(r,'THREE_MONTHS',now)).toContain('2027-01-08');expect(inventoryDates(r,'THREE_MONTHS',now)).not.toContain('2027-01-09');expect(inventoryDates(r,'SIX_MONTHS',now)).toContain('2027-01-09');});
it('counts series separately from OI and volume, preserving unknown fields',()=>{const t=inventoryTotals([c('2026-10-08'),c('2026-10-09','call',null),c('2026-10-08','put',50)]);expect(t.call.series).toBe(2);expect(t.call.oi).toBeNull();expect(t.call.volume).toBe(6);expect(t.put.oi).toBe(50);expect(t.call.oiKnown).toBe(1);});
it('accepts a valid zero-sided or empty chain without inventing options',()=>{const r=report();expect(validInventory(r,'QQQ',now)).toBe(true);expect(inventoryTotals([]).call).toMatchObject({series:0,oi:0,volume:0});r.enrichment!.details=[];r.contracts=0;expect(validInventory(r,'QQQ',now)).toBe(true);});
it.each(['foreign','stale','future','duplicates','missing-series','beyond-six-months','negative-oi','bad-kind'])('rejects %s inventories',kind=>{const r=report();if(kind==='foreign')r.symbol='SPY';if(kind==='stale')r.at-=300001;if(kind==='future')r.at++;if(kind==='duplicates')r.enrichment!.details[1]={...r.enrichment!.details[0]};if(kind==='missing-series')r.contracts++;if(kind==='beyond-six-months')r.enrichment!.details[0].expiration='2027-04-09';if(kind==='negative-oi')r.enrichment!.details[0].oi=-1;if(kind==='bad-kind')r.enrichment!.details[0].kind='unknown';expect(validInventory(r,'QQQ',now)).toBe(false);});
it('places full chain in Radar scanner and keeps it out of the chart',()=>{expect(gammaRouteChart(undefined,'QQQ',false,now)).not.toContain('data-contract-inventory');const html=contractChainPanel();expect(html).toContain('data-contract-inventory');expect(html).toContain('Todos · 6 meses');expect(html).toContain('data-inventory-expiry');expect(html).not.toContain('TOTAL DEL FILTRO');expect(inventoryPanel('<script>',now)).not.toContain('<script>');});
