import {it,expect} from 'vitest';
import {historicalAuction} from './auction-history';
import {auctionMapPanel} from './auction-map-panel';
import type {ChartHistory} from './candle-series';
const open=Date.parse('2026-10-09T09:30:00-04:00'),close=open+390*60000,now=close+86400000;
function history():ChartHistory{return {sessions:[{date:'2026-10-09',openMs:open,closeMs:close}],minutes:Array.from({length:390},(_,i)=>({timestampMs:open+i*60000,open:100+i*.01,close:100.01+i*.01,high:100.02+i*.01,low:99.99+i*.01,volume:100}))};}
it('uses the final regular-session close and conserves historical candle volume',()=>{
 const h=history(),before=JSON.stringify(h),r=historicalAuction('QQQ',h,now)!;expect(r.date).toBe('2026-10-09');expect(r.close).toBeCloseTo(103.9);expect(r.reading.totalVolume).toBeCloseTo(39000);expect(r.reading.bins.reduce((n,b)=>n+b.volume,0)).toBeCloseTo(39000);expect(r.reading.version).toBe('auction-map-bars-reference-v1');expect(r.reading.routes).toEqual([]);expect(JSON.stringify(h)).toBe(before);
});
it('does not call an incomplete final minute a session close',()=>{const h=history();h.minutes.pop();expect(historicalAuction('QQQ',h,now)).toBeUndefined();expect(historicalAuction('QQQ',history(),close-1)).toBeUndefined();});
it('does not build a session profile from missing opening or interior minutes',()=>{
 for(const index of [0,195]){const h=history();h.minutes.splice(index,1);expect(historicalAuction('QQQ',h,now)).toBeUndefined();}
});
it('accepts a complete early-close session without requiring 390 minutes',()=>{
 const h=history();h.sessions[0].closeMs=open+210*60000;h.minutes=h.minutes.slice(0,210);
 expect(historicalAuction('QQQ',h,now)!.reading.totalVolume).toBeCloseTo(21000);
});
it('rejects corrupt, duplicated and excessively old histories',()=>{const h=history();h.minutes[0].volume=NaN;expect(historicalAuction('QQQ',h,now)).toBeUndefined();const d=history();d.minutes.push({...d.minutes[0]});expect(historicalAuction('QQQ',d,now)).toBeUndefined();expect(historicalAuction('QQQ',history(),close+15*86400000)).toBeUndefined();});
it('renders historical numbers while disconnected without pretending they are live',()=>{
 const r=historicalAuction('QQQ',history(),now)!;const html=auctionMapPanel('QQQ',false,now,undefined,null,r);expect(html).toContain('viernes');expect(html).toContain('103.90');expect(html).toContain('CIERRE DE LA SESIÓN');expect(html).toContain(' · estimada');expect(html).toContain('No es un perfil tick exacto');expect(html).not.toContain('PRECIO ACTUAL');expect(auctionMapPanel('SPY',false,now,undefined,null,r)).not.toContain('103.90');
});
