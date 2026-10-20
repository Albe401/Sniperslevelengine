import {it,expect} from 'vitest';
import {readFileSync} from 'node:fs';
import {savedReadingDisplay,savedReadingNotice,capturedReadingMarkup} from './saved-reading-display';
import {currentMapHorizon} from './reading-display';
import {displayStructure} from './reading-display';
import type {LiveState} from './live';
const at=Date.parse('2026-10-20T19:55:02.042Z');
const frame={symbol:'QQQ',publishedMs:at,optionSnapshotStatus:{phase:'ready',observedMs:at-60000,horizons:[{mode:'DAY',observedMs:at-60000,expirations:['2026-10-20'],structure:{complete:false,liquidityComplete:false,provisional:{coveredContracts:192,totalContracts:236,callWall:{strike:725},putWall:{strike:715},magnet:719}}}]}} as unknown as LiveState;
it('retains an old reference with its original clock and partial coverage',()=>{
 const s=savedReadingDisplay(frame,'QQQ',at,at+7200000);
 expect(s.valid).toBe(true);expect(s.current).toBe(false);expect(s.renderAt).toBe(at);expect(s.partial).toBe(true);expect(s.coverage).toBe('192/236 contratos incluidos');
 expect(currentMapHorizon(frame.optionSnapshotStatus,at+7200000)).toBeUndefined();
 expect(displayStructure(currentMapHorizon(frame.optionSnapshotStatus,s.renderAt),true)?.callWall?.strike).toBe(725);
 expect(savedReadingNotice(frame,'QQQ',at,at+7200000)).toContain('Referencia histórica');
 expect(savedReadingNotice(frame,'QQQ',at,at+7200000)).toContain('no es una señal vigente');
});
it('updates freshness without changing the archived clock across date boundaries',()=>{
 expect(savedReadingDisplay(frame,'QQQ',at,at).current).toBe(true);
 expect(savedReadingDisplay(frame,'QQQ',at,at+86400000).current).toBe(false);
 expect(savedReadingDisplay(frame,'QQQ',at,at+86400000).renderAt).toBe(at);
});
it('refuses foreign, future, malformed or incorrectly ordered captures',()=>{
 for(const s of [savedReadingDisplay(frame,'SPY',at,at),savedReadingDisplay(frame,'QQQ',at,at-1),savedReadingDisplay(frame,'QQQ',NaN,at),savedReadingDisplay({...frame,publishedMs:at+1},'QQQ',at,at+1000)])expect(s.valid).toBe(false);
 expect(savedReadingDisplay({...frame,optionSnapshotStatus:undefined},'QQQ',at,at+7200000).coverage).toBe('');
});
it.skipIf(!process.env.SNIPERS_WEB_REAL_FRAME)('preserves levels from the real public QQQ snapshot',()=>{
 const d=JSON.parse(readFileSync(process.env.SNIPERS_WEB_REAL_FRAME!,'utf8'));
 const result=savedReadingDisplay(d.frame,'QQQ',d.capturedMs,d.capturedMs+7200000);
 expect(result.valid).toBe(true);expect(result.current).toBe(false);expect(result.partial).toBe(true);
 const original=d.frame.optionSnapshotStatus.horizons.find((h:{mode:string})=>h.mode==='DAY').structure.provisional;
 const rendered=displayStructure(currentMapHorizon(d.frame.optionSnapshotStatus,result.renderAt),true);
 expect(rendered?.callWall?.strike).toBe(original.callWall.strike);expect(rendered?.putWall?.strike).toBe(original.putWall.strike);expect(rendered?.magnet).toBe(original.magnet);
});

it('labels the stored desktop markup as a capture without changing selectors or values',()=>{expect(capturedReadingMarkup('<section data-live-view="detailed">en vivo provisional · PRUEBA EN VIVO · PRECIO ACTUAL 725.00</section>')).toBe('<section data-live-view="detailed">en la captura provisional · PRUEBA en la captura · PRECIO EN CAPTURA 725.00</section>');});
