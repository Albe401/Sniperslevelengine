import {it,expect} from 'vitest';
import {structuralCards,structuralMapPreview,structuralAlignment} from './structural-map-preview';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,5,15);
it('shows partial shares in the circle without certifying structural direction',()=>{const s=state();Object.assign(s.optionSnapshotStatus.horizons[0].exposure,{dexComplete:false,eligible:3,deltaUsed:2,callDexPct:null,putDexPct:null});const cards=structuralCards(s,'QQQ',true,now);expect(cards[0].call).toBe(80);expect(cards[0].partial).toBe(true);expect(cards[0].side).toBe('PENDIENTE');expect(structuralAlignment(cards).label).toContain('PENDIENTE');const html=structuralMapPreview('QQQ',s,true,now);expect(html).toContain('class="horizon call partial"');expect(html).toContain('style="--share:80"');expect(html).toContain('<strong>80%</strong><span>PARCIAL</span>');expect(html).toContain('Muestra parcial: CALL 80% / PUT 20% · cobertura delta 67%. No confirma dirección.');expect(html).toContain('CALLS*');expect(html).toContain('PUTS*');});
it('colors partial PUT dominance red while keeping the alignment pending',()=>{const s=state();Object.assign(s.optionSnapshotStatus.horizons[0].exposure,{dexComplete:false,eligible:3,deltaUsed:2,callDex:300,putDex:-700,netDex:-400,callDexPct:null,putDexPct:null});const cards=structuralCards(s,'QQQ',true,now);expect(cards[0].side).toBe('PENDIENTE');const html=structuralMapPreview('QQQ',s,true,now);expect(html).toContain('class="horizon put partial"');expect(html).toContain('style="--share:70"');expect(html).toContain('<strong>70%</strong><span>PARCIAL</span>');});
it('keeps the structural header compact while retaining pending horizon evidence',()=>{
 const s=state();const start=now-6*60000;
 s.ticks={QQQ:{symbol:'QQQ',price:106.3,tradeMs:now,bid:null,ask:null,quoteMs:null}} as LiveState['ticks'];s.options={phase:'denied'} as LiveState['options'];
 s.chartHistory={sessions:[{date:'2026-10-05',openMs:Date.UTC(2026,9,5,13,30),closeMs:Date.UTC(2026,9,5,20)}],minutes:[101,102,103,104,105,106].map((close,i)=>({timestampMs:start+i*60000,open:close-.2,high:close+.3,low:close-.3,close,volume:1000}))} as LiveState['chartHistory'];
 Object.assign(s.optionSnapshotStatus.horizons[0].exposure,{dexComplete:false,eligible:3,deltaUsed:2,callDexPct:null,putDexPct:null});
 const html=structuralMapPreview('QQQ',s,true,now);
 expect(html).toContain('<strong>MAPA ESTRUCTURAL</strong>');expect(html).not.toContain('PRECIO SUBIENDO');expect(html).not.toContain('OPCIONES PENDIENTES');expect(html).toContain('Muestra parcial: CALL');
});
const period={buy:800,sell:100,unknown:100,delta:700,coveragePct:90,lastMs:now,sinceMs:now-60000,changeOne:100,changeFive:700,gaps:0};
function state(){return {symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},metrics:{session:'2026-10-05',phase:'Sesión RTH'},cvd:{symbol:'QQQ',date:'2026-10-05',at:now,available:true,premarket:{...period},regular:{...period},rejected:0},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:['DAY','WEEK','NEXT4'].map(mode=>({mode,observedMs:now,expirations:mode==='NEXT4'?['2026-10-05','2026-10-06','2026-10-07','2026-10-08']:['2026-10-05'],exposure:{eligible:2,deltaUsed:2,unknownOi:0,dexComplete:true,callDex:800,putDex:-200,netDex:600,callDexPct:80,putDexPct:20}}))}} as unknown as LiveState}
it('uses each real horizon and CVD without changing exposure percentages',()=>{const s=state();s.optionSnapshotStatus.horizons[1].exposure={...s.optionSnapshotStatus.horizons[1].exposure,callDex:200,putDex:-800,netDex:-600,callDexPct:20,putDexPct:80};const r=structuralCards(s,'QQQ',true,now);expect(r.map(x=>x.call)).toEqual([80,20,80]);expect(r.map(x=>x.cvd.status)).toEqual(['confirms','contradicts','confirms']);expect(r[2].detail).not.toContain('menos de 4');expect(r[1].cvd.detail).toContain('no CVD acumulado semanal');});
it.each(['inactive','symbol','stale','future','partial','nan','sum','percentage','coverage'])('rejects %s structural evidence',kind=>{const s=state(),e=s.optionSnapshotStatus.horizons[0].exposure;if(kind==='symbol')s.symbol='SPY';if(kind==='stale')s.optionSnapshotStatus.observedMs=now-900001;if(kind==='future')s.optionSnapshotStatus.horizons[0].observedMs=now+1;if(kind==='partial')e.dexComplete=false;if(kind==='nan')e.callDex=NaN;if(kind==='sum')e.netDex=10;if(kind==='percentage')e.callDexPct=70;if(kind==='coverage')e.deltaUsed=1;expect(structuralCards(s,'QQQ',kind!=='inactive',now)[0].call).toBeNull();});
it('preserves valid structural exposure when CVD becomes stale',()=>{const s=state();s.cvd!.at=now-3001;const r=structuralCards(s,'QQQ',true,now);expect(r[0].call).toBe(80);expect(r.every(x=>x.cvd.status==='pending')).toBe(true);});
it('shows next session explicitly and never substitutes WEEK for DAY',()=>{const s=state();s.optionSnapshotStatus.horizons[0].mode='NEXT_SESSION';s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];expect(structuralCards(s,'QQQ',true,now)[0].detail).toContain('Próxima sesión');s.optionSnapshotStatus.horizons.shift();expect(structuralCards(s,'QQQ',true,now)[0].call).toBeNull();});
it('removes examples and escapes labels',()=>{const html=structuralMapPreview('<img>',undefined,false,now);expect(html).not.toContain('<img>');expect(html).not.toContain('68%');expect(html).not.toContain('DATOS DE EJEMPLO');expect(html.match(/CVD · PENDIENTE/g)).toHaveLength(3);});

it('matches the backend neutral band rather than declaring direction at 51%',()=>{const s=state();s.optionSnapshotStatus.horizons[0].exposure={...s.optionSnapshotStatus.horizons[0].exposure,callDex:510,putDex:-490,netDex:20,callDexPct:51,putDexPct:49};const r=structuralCards(s,'QQQ',true,now)[0];expect(r.side).toBe('NEUTRAL');expect(r.cvd.status).toBe('pending');});
it('confirms only coherent CALL or PUT exposure across all three horizons',()=>{
 const s=state();expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toBe('DELTA CALL · 3/3');
 for(const h of s.optionSnapshotStatus.horizons)Object.assign(h.exposure,{callDex:200,putDex:-800,netDex:-600,callDexPct:20,putDexPct:80});
 expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toBe('DELTA PUT · 3/3');
 Object.assign(s.optionSnapshotStatus.horizons[1].exposure,{callDex:800,putDex:-200,netDex:600,callDexPct:80,putDexPct:20});
 expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toBe('DELTA MIXTO');
});
it.each([0,1,2,3,5])('never certifies NEXT4 from %i expiration dates',count=>{
 const s=state();s.optionSnapshotStatus.horizons[2].expirations=Array.from({length:count},(_,i)=>`2026-10-${String(5+i).padStart(2,'0')}`);
 const cards=structuralCards(s,'QQQ',true,now);expect(cards[2].call).toBeNull();expect(cards[2].side).toBe('PENDIENTE');expect(structuralAlignment(cards).label).toContain('PENDIENTE');
});
it.each(['duplicate-date','expired','invalid-date','unordered','wrong-day','wrong-week','duplicate-mode','mixed-snapshot','missing-delta','fractional-count','negative-count'])('keeps %s evidence from being three aligned horizons',kind=>{
 const s=state(),h=s.optionSnapshotStatus.horizons[2];
 if(kind==='duplicate-date')h.expirations[3]=h.expirations[2];if(kind==='expired')h.expirations[0]='2026-10-02';if(kind==='invalid-date')h.expirations[0]='2026-02-30';if(kind==='unordered')h.expirations.reverse();if(kind==='wrong-day')s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];if(kind==='wrong-week')s.optionSnapshotStatus.horizons[1].expirations=['2026-10-12'];if(kind==='duplicate-mode')s.optionSnapshotStatus.horizons.push({...h});if(kind==='mixed-snapshot')h.observedMs=now-1000;if(kind==='missing-delta')h.exposure.deltaUsed=1;if(kind==='fractional-count'){h.exposure.eligible=2.5;h.exposure.deltaUsed=2.5;}if(kind==='negative-count'){h.exposure.eligible=-2;h.exposure.deltaUsed=-2;}
 expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toContain('PENDIENTE');
});
it('checks each horizon against the selected expiration universe',()=>{
 const s=state();s.optionSnapshotStatus.expirations=['2026-10-05','2026-10-06','2026-10-07','2026-10-08','2026-10-09'];
 expect(structuralCards(s,'QQQ',true,now)[1].side).toBe('PENDIENTE');
 s.optionSnapshotStatus.horizons[1].expirations=[...s.optionSnapshotStatus.expirations];expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toBe('DELTA CALL · 3/3');
 s.optionSnapshotStatus.horizons[2].expirations=['2026-10-06','2026-10-07','2026-10-08','2026-10-09'];expect(structuralAlignment(structuralCards(s,'QQQ',true,now)).label).toContain('PENDIENTE');
});
it('shows next expiration explicitly rather than labeling it today',()=>{
 const s=state();s.optionSnapshotStatus.horizons[0].mode='NEXT_SESSION';s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];
 const cards=structuralCards(s,'QQQ',true,now);expect(cards[0].name).toBe('PRÓX. VENC.');expect(cards[0].call).toBe(80);expect(structuralMapPreview('QQQ',s,true,now)).toContain('PRÓX. VENC.');
});
it.each([47.5,50,52.5])('respects the exact neutral-band edge at %f',call=>{
 const s=state();for(const h of s.optionSnapshotStatus.horizons)Object.assign(h.exposure,{callDex:call*10,putDex:-(100-call)*10,netDex:(2*call-100)*10,callDexPct:call,putDexPct:100-call});
 expect(structuralCards(s,'QQQ',true,now)[0].side).toBe(call===50?'NEUTRAL':call===52.5?'CALL':'PUT');
});
it('does not turn disconnected or opposing CVD into a different structural side',()=>{
 const s=state(),base=structuralCards(s,'QQQ',true,now);s.cvd!.regular={...period,buy:100,sell:800,delta:-700};
 const opposed=structuralCards(s,'QQQ',true,now);expect(opposed.map(c=>c.call)).toEqual(base.map(c=>c.call));expect(opposed.every(c=>c.cvd.status==='contradicts')).toBe(true);expect(structuralAlignment(opposed).label).toBe('DELTA CALL · 3/3');
 s.cvd!.available=false;expect(structuralCards(s,'QQQ',true,now).every(c=>c.cvd.status==='pending')).toBe(true);
});
it('benchmarks 1000 full structural-map renders',()=>{
 const s=state(),start=performance.now();for(let i=0;i<1000;i++){const html=structuralMapPreview('QQQ',s,true,now);expect(html).toContain('<strong>MAPA ESTRUCTURAL</strong>');expect(html).toContain('class="horizons"');}
 const elapsed=performance.now()-start;console.info(`MAPA ESTRUCTURAL: 1000 renders ${elapsed.toFixed(1)} ms; ${(elapsed/1000).toFixed(3)} ms/actualización`);expect(elapsed).toBeLessThan(10000);
},15000);
