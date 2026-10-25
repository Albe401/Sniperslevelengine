import {it,expect} from 'vitest';
import {readMassiveExposure} from './massive-exposure';
import {structuralIndicators,validStructuralIndicators} from './structural-indicators';
import type {LiveState} from './live';
import type {PriceBar} from './price-structure';
const now=Date.parse('2026-10-23T15:00:00-04:00');
function state():LiveState{return {symbol:'QQQ',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',expirations:['2026-10-23'],observedMs:now,exposure:{gammaUsed:1,eligible:1,gexComplete:true,unknownOi:0,unsupported:0,netGex:10,strikes:[{strike:700,callGex:20,putGex:10,netGex:10}]}}]}} as unknown as LiveState;}
it('audit rejects a GEX row with only one declared CALL/PUT component',()=>{const s=state();delete (s.optionSnapshotStatus.horizons[0].exposure.strikes[0] as any).callGex;expect(readMassiveExposure(s,'QQQ',true,now).reason).not.toBe('');});
it('audit rejects a nonexistent expiry even when its string sorts after today',()=>{const s=state();s.optionSnapshotStatus.horizons[0].mode='NEXT_SESSION';s.optionSnapshotStatus.horizons[0].expirations=['2026-13-31'];expect(readMassiveExposure(s,'QQQ',true,now).reason).not.toBe('');});
it.each(['unknownOi','unsupported'])('audit cannot mark GEX complete with declared %s',field=>{const s=state();(s.optionSnapshotStatus.horizons[0].exposure as any)[field]=1;expect(readMassiveExposure(s,'QQQ',true,now).complete).toBe(false);});
it.each(['emaSeedAtr','adxSeedPoints'])('audit rejects a negative absolute sensitivity: %s',field=>{const bars:PriceBar[]=Array.from({length:300},(_,i)=>({timestampMs:now-(300-i)*900000,endMs:now-(299-i)*900000,sessionDate:'2026-10-23',open:100+i/10,high:101+i/10,low:99+i/10,close:100+i/10,volume:100}));const v=structuralIndicators(bars,15,2);expect(v.ready).toBe(true);expect(validStructuralIndicators(v,now,300)).toBe(true);(v as any)[field]=-0.5;expect(validStructuralIndicators(v,now,300)).toBe(false);});
