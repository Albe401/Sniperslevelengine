import {expect,it} from 'vitest';
import {dayMap,indexedLevelVolume} from './day-map';
import {levelOpenInterest} from './level-open-interest';
import {strikeKey} from './strike-key';
import {mapDataQuality} from './map-data-quality';
import type {Horizon} from './live';
import type {EtfReport} from './etf-dealers';
import type {ContractWindow} from './flow-coverage';
const h={mode:'DAY',expirations:['2026-10-13'],structure:{complete:true,selected:{},callWall:{strike:100,score:80},routes:[{direction:'CALL',barrier:101,barrierScore:60}]}} as unknown as Horizon;
const oiRows=['call','put'].flatMap((kind,i)=>[{ticker:kind,kind,strike:101,expiration:'2026-10-13',oi:i?300:100},{ticker:kind+'future',kind,strike:101,expiration:'2026-10-14',oi:9999}]);
const report={expirations:['2026-10-13','2026-10-14'],enrichment:{details:oiRows}} as unknown as EtfReport;
it('barrier flow remains primary even with opposite snapshot OI and exposes buying',()=>{
 const before=JSON.stringify(report);const html=dayMap(h,true,[],true,99,'',undefined,true,'',undefined,undefined,'',{oiReport:report,barrierRows:['call','put'].map((kind,i)=>({ticker:kind,kind,strike:101,expiration:'2026-10-13',covered:true,ask:i?5:60,bid:i?5:30,unknown:0}))});
 expect(html).toContain('<span>Flujo 5 min · CALL 90% · PUT 10%</span>');expect(html).toContain('Compra 65% · Venta 35%');expect(html).toContain('CALL 100 · PUT 300');expect(html).not.toContain('9999');expect(JSON.stringify(report)).toBe(before);expect(html).toContain('Delta nivel−precio:');
});
it.each([NaN,Infinity,-Infinity,0,-10])('rejects invalid spot %s without rendering a false distance',spot=>{const html=dayMap(h,true,[],false,spot);expect(html).toContain('Sin precio reciente');expect(html).not.toMatch(/NaN|Infinity|Delta nivel−precio:/);});
it('OI scopes expiry, requires each side, and tolerates floating point representation only',()=>{expect(levelOpenInterest(report,101.00000001,['2026-10-13'])).toMatchObject({total:400,expirations:['2026-10-13']});expect(levelOpenInterest(report,101,[])).toBeNull();expect(levelOpenInterest(report,101,['2026-10-15'])).toBeNull();expect(strikeKey(101.0004)).toBeNull();expect(strikeKey(Infinity)).toBeNull();});
it('indexed volume preserves duplicate rejection, zero and exact expiry',()=>{const rows=['CALL','PUT'].map((kind,i)=>({ticker:kind,kind,strike:101.00000001,expiration:'2026-10-13',minutes:1,covered:true,contracts:i?4:6})) as ContractWindow[];expect(indexedLevelVolume(rows)(101,['2026-10-13'],1,true)).toBe(10);expect(indexedLevelVolume([...rows,rows[0]])(101,['2026-10-13'],1,true)).toBeNull();expect(indexedLevelVolume(rows.map(r=>({...r,contracts:0})))(101,['2026-10-13'],1,true)).toBe(0);});
it('never certifies temporal coherence with missing timestamps',()=>{const common={kind:'observed' as const,source:'test',method:'test',available:true};expect(mapDataQuality([{...common,name:'A',asOf:1000},{...common,name:'B',asOf:null}]).complete).toBe(false);expect(mapDataQuality([{...common,name:'A',asOf:1000},{...common,name:'B',asOf:2000}])).toMatchObject({status:'MIXED-AS-OF',skewMs:1000});});
