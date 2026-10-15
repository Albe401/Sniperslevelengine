import {it,expect} from 'vitest';
import {chartBigTrades} from './big-trades';
import type {CvdRead} from './cvd';
const now=Date.UTC(2026,9,9,14),p={at:now-60000,price:100,shares:2000,notional:200000,side:1,sequence:5};
const read={symbol:'QQQ',date:'2026-10-09',at:now,bigTrades:[p]} as CvdRead;
it('uses actual individual stock notional and preserves quote-inferred or unknown side',()=>{
 expect(chartBigTrades(read,'QQQ',100000,now)).toEqual([p]);
 expect(chartBigTrades(read,'QQQ',250000,now)).toEqual([]);
 expect(chartBigTrades({...read,bigTrades:[{...p,side:0}]},'QQQ',100000,now)[0].side).toBe(0);
});
it('rejects foreign, stale, duplicate, fabricated and future prints',()=>{
 expect(chartBigTrades(read,'IWM',100000,now)).toEqual([]);
 expect(chartBigTrades(read,'QQQ',100000,now+4000)).toEqual([]);
 expect(chartBigTrades({...read,bigTrades:[p,p,{...p,sequence:6,notional:999999},{...p,sequence:7,at:now+1001}]},'QQQ',100000,now)).toEqual([p]);
});
