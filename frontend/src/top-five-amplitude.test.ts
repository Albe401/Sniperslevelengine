import {it,expect} from 'vitest';
import {topFiveAmplitude} from './top-five-amplitude';
import type {EtfReport} from './etf-dealers';
const now=Date.parse('2026-10-02T15:00:00Z');
const report=()=>({symbol:'AAPL',at:now,spot:100,dailyRange:{ready:true,spot:100,expectedMoveDollars:10,expectedMovePct:10,snapshotLower:90,snapshotUpper:110,method:'ATM_IV_1D'}} as EtfReport);
it('shows a symmetric one-day estimate with dollars percentage and bounds',()=>{const html=topFiveAmplitude(report(),now);expect(html).toContain('CALL +$10.00 · 10.00%');expect(html).toContain('PUT −$10.00 · 10.00%');expect(html).toContain('$90.00–$110.00');expect(html).toContain('puede superarse');});
it('shows asymmetric remaining room only with a matching fresh RTH anchor',()=>{const r=report();r.amplitude={symbol:'AAPL',at:now,available:true,anchored:true,callDollars:7,putDollars:13,callPct:7/103*100,putPct:13/103*100,reference:100,lower:90,upper:110,extensionCall:0,extensionPut:0,expectedMoveDollars:10,expectedMovePct:10,reason:''};expect(topFiveAmplitude(r,now)).toContain('CALL +$7.00');expect(topFiveAmplitude(r,now)).toContain('PUT −$13.00');r.amplitude.symbol='QQQ';expect(topFiveAmplitude(r,now)).not.toContain('RESTANTE');r.at=now-300001;expect(topFiveAmplitude(r,now)).toContain('CALL —');});
it('rejects inconsistent or unavailable estimates',()=>{const r=report();r.dailyRange!.expectedMovePct=80;expect(topFiveAmplitude(r,now)).toContain('CALL —');expect(topFiveAmplitude(undefined,now)).toContain('esperando IV');});
