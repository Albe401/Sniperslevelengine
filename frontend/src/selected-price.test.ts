import {describe,it,expect} from 'vitest';
import {selectedPrice,selectedPriceMarkup} from './selected-price';
import type {LiveState} from './live';
const now=1800000000000;
function fixture(){return {symbol:'QQQ',ticks:{QQQ:{symbol:'QQQ',price:501.25,tradeMs:now-1000,bid:null,ask:null,quoteMs:null}},stocks:{phase:'streaming'},publishedMs:now,chartHistory:{minutes:[],sessions:[]}} as unknown as LiveState;}
describe('selected asset quote',()=>{
 it('shows fresh trades with the selected ticker',()=>{const q=selectedPrice('QQQ',fixture(),true,now);expect(q.live).toBe(true);expect(selectedPriceMarkup(q)).toContain('$501.25');expect(selectedPriceMarkup(q)).toContain('QQQ');});
 it('keeps an old trade visible but never calls it live',()=>{const s=fixture();s.ticks.QQQ.tradeMs=now-60000;expect(selectedPrice('QQQ',s,true,now)).toMatchObject({value:501.25,live:false,label:'Última operación'});expect(selectedPrice('QQQ',fixture(),false,now).live).toBe(false);});
 it('never reuses another selected asset price',()=>{expect(selectedPrice('AAPL',fixture(),true,now).value).toBeNull();const s=fixture();s.ticks.QQQ.symbol='AAPL';expect(selectedPrice('QQQ',s,true,now).value).toBeNull();});
 it('uses the latest completed history minute and labels its source',()=>{const s=fixture();s.ticks={};s.chartHistory!.minutes=[{timestampMs:now-120000,open:490,high:500,low:489,close:499,volume:10},{timestampMs:now-60000,open:499,high:502,low:498,close:500,volume:10},{timestampMs:now-500,open:500,high:510,low:498,close:509,volume:10}];expect(selectedPrice('QQQ',s,true,now)).toMatchObject({value:500,at:now,live:false,label:'Cierre de vela · 1 min'});});
 it('rejects invalid or future prices and shows a useful empty state',()=>{for(const n of [NaN,Infinity,0,-1]){const s=fixture();s.ticks.QQQ.price=n;expect(selectedPrice('QQQ',s,true,now).value).toBeNull();}const s=fixture();s.ticks.QQQ.tradeMs=now+1001;expect(selectedPrice('QQQ',s,true,now).value).toBeNull();expect(selectedPriceMarkup(selectedPrice('QQQ',undefined,false,now))).toContain('Mercado en vivo');});
 it('does not call a quote live when the feed snapshot is stale',()=>{const s=fixture();s.publishedMs=now-10000;expect(selectedPrice('QQQ',s,true,now).live).toBe(false);});
});
