import {expect,it} from 'vitest';
import {currentMapHorizon} from './reading-display';
import {readPriceLevels} from './price-levels';
import {readIntradayMap} from './intraday-map';
import {mapRouteLevels} from './route-level-context';
import {gammaLiveModel} from './gamma-live-model';
import {dayMap} from './day-map';
import type {LiveState} from './live';
import type {PriceNow} from './price-now';
const now=Date.parse('2026-10-05T15:00:00Z');
const p={ready:true,spot:100,direction:1} as PriceNow;
function fixture(){return {symbol:'QQQ',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-05'],exposure:{gexComplete:true},structure:{complete:true,gexAvailable:true,selected:{M1:100},magnet:100,callWall:{strike:101,score:82},putWall:{strike:99,score:43},routes:[],strikeRows:[99,100,101].map(strike=>({strike,call_oi:10,put_oi:10,total_oi:20,call_gex:20,put_gex:10,net_gex:10,gross_gex:30,total_gamma_weight:2,total_volume:100,score:60}))}}]}} as unknown as LiveState;}
it.each(['mixed snapshot','duplicate horizon','expired expiry','future expiry','multiple expiries','missing timestamp','prior day'])('withdraws wall references consistently: %s',kind=>{
 const s=fixture(),h=s.optionSnapshotStatus.horizons[0];
 if(kind==='mixed snapshot')h.observedMs--;
 if(kind==='duplicate horizon')s.optionSnapshotStatus.horizons.push({...h});
 if(kind==='expired expiry')h.expirations=['2026-10-02'];
 if(kind==='future expiry')h.expirations=['2026-10-06'];
 if(kind==='multiple expiries')h.expirations.push('2026-10-06');
 if(kind==='missing timestamp')h.observedMs=NaN;
 if(kind==='prior day'){h.observedMs=s.optionSnapshotStatus.observedMs=Date.parse('2026-10-02T20:00:00Z');}
 expect(currentMapHorizon(s.optionSnapshotStatus,now)).toBeUndefined();
 expect(readPriceLevels(s,'QQQ',true,p,now).important).toBe('Sin niveles vigentes');
 expect(readIntradayMap(s,'QQQ',true,p,now).levels).toEqual([]);
 expect(mapRouteLevels(s,'QQQ',true,now)).toEqual([]);
 expect(gammaLiveModel(s,'QQQ',true,now).levels.every(l=>l.strike==null&&l.force==null)).toBe(true);
});
it('keeps exact wall scores consistent across important levels, intraday control, route and map',()=>{
 const s=fixture(),h=currentMapHorizon(s.optionSnapshotStatus,now)!;
 expect(readPriceLevels(s,'QQQ',true,p,now).important).toContain('CALL WALL $101.00 (82.0/100)');
 expect(readIntradayMap(s,'QQQ',true,p,now).levels.find(l=>l.name==='PUT WALL')?.score).toBe(43);
 expect(mapRouteLevels(s,'QQQ',true,now).find(l=>l.role==='call-wall')?.strength).toBe(82);
 const html=dayMap(h,true,[],false,100);
 expect(html).toContain('82.0/100 · MUY ALTA');expect(html).toContain('43.0/100 · MEDIA');
 // A distinct gamma/GEX index does not overwrite the canonical wall scores.
 expect(gammaLiveModel(s,'QQQ',true,now).levels.find(l=>l.id==='call')?.force).toBeCloseTo(100);
});
it.each([-1,101,Infinity,NaN])('never certifies malformed wall score %s',bad=>{
 const s=fixture();s.optionSnapshotStatus.horizons[0].structure.callWall!.score=bad;
 expect(readIntradayMap(s,'QQQ',true,p,now).levels.find(l=>l.name==='CALL WALL')?.score).toBeUndefined();
 expect(mapRouteLevels(s,'QQQ',true,now).find(l=>l.role==='call-wall')?.strength).toBeNull();
 expect(readPriceLevels(s,'QQQ',true,p,now).dominant).toContain('PUT WALL');
});
it('rejects gamma normalization overflow instead of returning NaN strength',()=>{
 const s=fixture(),r=s.optionSnapshotStatus.horizons[0].structure.strikeRows![0];
 r.call_oi=r.put_oi=1e-308;r.total_oi=2e-308;r.total_gamma_weight=1e308;
 expect(gammaLiveModel(s,'QQQ',true,now).levels.every(l=>l.force==null&&!l.dominant)).toBe(true);
});
it('next-session references remain explicit and never become live DAY confirmation',()=>{
 const s=fixture(),h=s.optionSnapshotStatus.horizons[0];h.mode='NEXT_SESSION';h.expirations=['2026-10-06'];
 expect(currentMapHorizon(s.optionSnapshotStatus,now)).toBe(h);
 expect(readPriceLevels(s,'QQQ',true,p,now).quality).toContain('Próximo vencimiento');
 expect(readIntradayMap(s,'QQQ',true,p,now).levels).toEqual([]);
 expect(mapRouteLevels(s,'QQQ',true,now)).toEqual([]);
 expect(gammaLiveModel(s,'QQQ',true,now).levels.every(l=>l.force==null)).toBe(true);
});
it('benchmarks the three wall-reading consumers over repeated updates',()=>{
 const s=fixture(),runs=1000,start=performance.now();
 for(let i=0;i<runs;i++){readPriceLevels(s,'QQQ',true,p,now);readIntradayMap(s,'QQQ',true,p,now);mapRouteLevels(s,'QQQ',true,now);}
 const ms=(performance.now()-start)/runs;
 console.log(`Wall reading audit: ${ms.toFixed(3)} ms/update (1000 updates)`);
 expect(ms).toBeLessThan(5);
});
