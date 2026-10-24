import {it,expect} from 'vitest';
import type {LiveState} from './live';
import {intradayAtr} from './intraday-atr';
import {atrContext,structuralAtr} from './atr-context';
import {buildIntradayControl,ControlTracker} from './intraday-control';
import {quickReadingPanels} from './quick-reading-panels';
import {readDirectionScore} from './direction-scanner';
import {structuralCards} from './structural-map-preview';
import type {EtfDetail} from './etf-enrichment';
const open=Date.UTC(2026,9,5,13,30),now=open+90*60000;
it('renders the structural map beneath the visual auction map while disconnected',()=>{
 const html=quickReadingPanels(undefined,'QQQ',false,buildIntradayControl(undefined,'QQQ',false,now),now);
 const control=html.indexOf('aria-label="Mapas del activo"');
 const structure=html.indexOf('<strong>MAPA ESTRUCTURAL</strong>',control);
 const auction=html.indexOf('<strong>MAPA DE ACEPTACIÓN Y RECHAZO</strong>',control);
 expect(control).toBeGreaterThan(-1);expect(auction).toBeGreaterThan(control);expect(structure).toBeGreaterThan(auction);expect(html).toContain('PERFIL DE VOLUMEN');
 expect(html.slice(auction)).toContain('Pendiente de perfil del subyacente');
 for(const removed of ['ACTIVIDAD INUSUAL','Control de dealers','Riesgo de expansión de volatilidad','Engranaje con lectura rápida','EXTENSIÓN DEL PRECIO'])expect(html).not.toContain(removed);
});
function fixture():LiveState {
 const rows=Array.from({length:90},(_,i)=>{const c=100+i*.01;return {timestampMs:open+i*60000,open:c-.01,high:c+.2,low:c-.2,close:c,volume:100,vwap:c};});
 const bucket=(premium:number)=>({premium,contracts:10,trades:2});
 const windows=[1,5].map(minutes=>({minutes,event_count:5,contracts:20,premium:1000,premium_complete:true,bullish_pct:90,bearish_pct:10,direction:'BULLISH FLOW',buckets:{'CALL BUY':bucket(900),'PUT BUY':bucket(100),'CALL SELL':bucket(0),'PUT SELL':bucket(0),UNKNOWN:bucket(0)}}));
 const period={buy:800,sell:100,unknown:100,delta:700,coveragePct:90,lastMs:now,sinceMs:open,changeOne:100,changeFive:700,gaps:0};
 return {symbol:'QQQ',publishedMs:now,startedMs:open,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{symbol:'QQQ',price:100.89,tradeMs:now}},bars:{},minutes:Object.fromEntries(rows.map(r=>[r.timestampMs,r])),metrics:{session:'2026-10-05',phase:'Sesión RTH',vwap:100,rvol:1.2,asofMs:now},chartHistory:{sessions:[{date:'2026-10-02',openMs:Date.UTC(2026,9,2,13,30),closeMs:Date.UTC(2026,9,2,20)},{date:'2026-10-05',openMs:open,closeMs:open+390*60000}],minutes:rows},dailyAtr:{symbol:'QQQ',through:'2026-10-02',value:4,close:100,detail:'ATR14 diario de sesiones completas'},flow:{asof_ms:now,intraday_windows:windows,contract_windows:[]},cvd:{symbol:'QQQ',date:'2026-10-05',at:now,available:true,premarket:{...period},regular:{...period},recentFive:{buy:800,sell:100,unknown:100,delta:700,startMs:now-300000,endMs:now,complete:true},rejected:0},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:['DAY','WEEK','NEXT4'].map(mode=>({mode,observedMs:now,expirations:mode==='NEXT4'?['2026-10-05','2026-10-06','2026-10-07','2026-10-08']:['2026-10-05'],exposure:{eligible:2,deltaUsed:2,unknownOi:0,dexComplete:true,callDex:800,putDex:-200,netDex:600,callDexPct:80,putDexPct:20},structure:{complete:true,selected:{M1:102},callWall:{strike:103,score:80,ready:true},putWall:{strike:99,score:80,ready:true},routes:[],top:[]}}))}} as unknown as LiveState;
}
it('audits full renders with 2048 enriched contracts and 5090 minutes without contaminating readings',()=>{
 const s=fixture(),dates=['2026-10-05','2026-10-06','2026-10-07','2026-10-08'];
 s.chartHistory!.minutes.unshift(...Array.from({length:5000},(_,i)=>({...s.chartHistory!.minutes[0],timestampMs:open-(5000-i)*60000})));
 const details:EtfDetail[]=dates.flatMap(expiration=>Array.from({length:256},(_,i)=>['call','put'].map(kind=>({ticker:`${expiration}-${i}-${kind}`,expiration,kind,strike:90+i*.1,oi:100+i,oiChange:2,volume:20,volumeOi:20/(100+i),delta:kind==='call'?.5:-.5,gamma:.01,dex:(100+i)*50,gex:(100+i)*100,iv:.2,theta:-.1,vega:.1,charm:.001,vanna:.002,bid:1,ask:1.1,spreadPct:10,quoteAt:now,realtime:true,fresh:true})))).flat();
 s.optionSnapshotStatus.expirations=dates;
 for(const h of s.optionSnapshotStatus.horizons)h.expirations=h.mode==='DAY'?[dates[0]]:dates;
 s.optionSnapshotStatus.structuralEnrichment={symbol:'QQQ',days:28,at:now,spot:100.89,priceAt:now,expirations:dates,contracts:details.length,rows:[],gammaComplete:true,deltaComplete:true,enrichment:{details,oiReferenceDate:null,previousObservedDate:null,oiComparable:false,priceFresh:true,oiKnown:details.length,gammaKnown:details.length,deltaKnown:details.length,quotesFresh:details.length,storageError:null,model:'BSM'}};
 const before=JSON.stringify(s),tracker=new ControlTracker(),durations:number[]=[];
 for(let i=0;i<40;i++){
  const start=performance.now(),r=tracker.update(buildIntradayControl(s,'QQQ',true,now),now),html=quickReadingPanels(s,'QQQ',true,r,now);
  durations.push(performance.now()-start);expect(html.match(/data-structural-open=/g)).toHaveLength(6);expect(html).toContain('auction-map-visual');expect(html).not.toMatch(/\b(?:NaN|Infinity|undefined)\b/);expect(r.marketEvidence!.score.call).not.toBeNull();
 }
 expect(JSON.stringify(s)).toBe(before);
 durations.sort((a,b)=>a-b);console.info('QUICK_STRESS',JSON.stringify({minutes:s.chartHistory!.minutes.length,contracts:details.length,updates:40,meanMs:durations.reduce((a,b)=>a+b,0)/40,p95Ms:durations[38],maxMs:durations[39]}));
 expect(durations[38]).toBeLessThan(150);
},20000);
it('normalizes closed price movement and actual RTH range, without claiming remaining capacity',()=>{
 const s=fixture(),a=intradayAtr(s,'QQQ',true,now),c=atrContext(s,'QQQ',true,a,now);
 expect(a.value).toBeGreaterThan(0);expect(c.moveFive).toBeCloseTo(.05/a.value!);expect(c.rthRangeDaily).toBeCloseTo(1.29/4);expect(c.cvd).toContain('alineados');expect(c.detail).toContain('no rango restante');
 s.cvd!.recentFive!.delta=-700;s.cvd!.recentFive!.buy=100;s.cvd!.recentFive!.sell=800;expect(atrContext(s,'QQQ',true,a,now).cvd).toContain('divergentes');
});
it.each(['offset','gap','duplicate','unknown','foreign','stale','forming','wrong-date'])('does not infer CVD/ATR agreement from %s evidence',kind=>{
 const s=fixture(),a=intradayAtr(s,'QQQ',true,now);
 if(kind==='offset')s.cvd!.recentFive!.endMs=now-1;if(kind==='gap')s.chartHistory!.minutes.splice(85,1);if(kind==='duplicate')s.chartHistory!.minutes.push({...s.chartHistory!.minutes[85]});if(kind==='unknown'){s.cvd!.recentFive!.unknown=2000;}if(kind==='foreign')s.cvd!.symbol='SPY';if(kind==='stale')s.publishedMs=now-3001;if(kind==='forming')a.endMs=now+1;if(kind==='wrong-date')s.cvd!.date='2026-10-02';
 const c=atrContext(s,'QQQ',true,a,now);expect(c.cvd).not.toContain('alineados');expect(c.cvd).not.toContain('divergentes');if(['gap','duplicate'].includes(kind))expect(c.rthRangeDaily).toBeNull();
});
it('uses daily ATR for each structural wall and preserves all exposure percentages',()=>{
 const s=fixture(),a=intradayAtr(s,'QQQ',true,now),before=structuralCards(s,'QQQ',true,now),after=structuralCards(s,'QQQ',true,now,a);
 expect(after.map(h=>h.call)).toEqual(before.map(h=>h.call));expect(after.map(h=>h.atr?.label)).toEqual(['0.53 ATR D','0.53 ATR D','0.53 ATR D']);expect(after[1].atr?.detail).toContain('no predice alcance');
 expect(structuralAtr(a,undefined,'CALL',100).label).toContain('PENDIENTE');s.ticks.QQQ.tradeMs=now-15001;expect(structuralCards(s,'QQQ',true,now,a).every(h=>h.atr?.label.includes('PENDIENTE'))).toBe(true);
});
it('keeps daily scale during intraday warmup without fabricating CVD agreement',()=>{
 const s=fixture(),early=open+30*60000;s.publishedMs=early;s.chartHistory!.minutes=s.chartHistory!.minutes.slice(0,30);
 const a=intradayAtr(s,'QQQ',true,early),c=atrContext(s,'QQQ',true,a,early);
 expect(a.value).toBeNull();expect(c.label).toBe('ATR D · $4.00');expect(c.cvd).toContain('pendientes');expect(c.rthRangeDaily).toBeCloseTo(.69/4);
});
it('renders the complete quick reading with one shared ATR and unchanged bias/control',()=>{
 const s=fixture(),r=buildIntradayControl(s,'QQQ',true,now),bias=readDirectionScore(s,r,true,now),baseline=JSON.stringify(r);
 const html=quickReadingPanels(s,'QQQ',true,r,now);
 for(const marker of ['route-volatility-meters','ATR14 · DIARIO','ATR14 · 5 MIN','0.53 ATR D','CVD / avance 5 min: alineados','Mapas del activo','PERFIL DE VOLUMEN','MAPA DE ACEPTACIÓN Y RECHAZO','CVD y compras','Mapa de recorrido'])expect(html).toContain(marker);
 expect(html).not.toContain('Motor CVD');
 expect(JSON.stringify(r)).toBe(baseline);expect(readDirectionScore(s,r,true,now).call).toBe(bias.call);expect(html).toContain('no confirma agotamiento');expect(html.match(/class="route-volatility"/g)).toHaveLength(1);
});
it.each(['inactive','foreign','stale'])('the whole reading keeps %s ATR pending',kind=>{
 const s=fixture();if(kind==='foreign')s.symbol='NVDA';if(kind==='stale')s.publishedMs=now-3001;
 const active=kind!=='inactive',html=quickReadingPanels(s,'QQQ',active,buildIntradayControl(s,'QQQ',active,now),now);
 expect(html).not.toContain('0.53 ATR D');expect(html).not.toContain('CVD / avance 5 min: alineados');expect(html).toContain('ATR · PENDIENTE');
});
it('benchmarks the whole quick-reading calculation and HTML with a larger history',()=>{
 const s=fixture();s.chartHistory!.minutes.unshift(...Array.from({length:5000},(_,i)=>({...s.chartHistory!.minutes[0],timestampMs:open-(5000-i)*60000})));
 const tracker=new ControlTracker(),durations:number[]=[];
 for(let i=0;i<300;i++){const start=performance.now();const r=tracker.update(buildIntradayControl(s,'QQQ',true,now),now);const html=quickReadingPanels(s,'QQQ',true,r,now);expect(html.length).toBeGreaterThan(10000);durations.push(performance.now()-start);}
 const elapsed=durations.reduce((a,b)=>a+b,0),ordered=durations.slice().sort((a,b)=>a-b);console.info(`LECTURA RÁPIDA COMPLETA + ATR, 5090 minutos: 300 actualizaciones ${elapsed.toFixed(1)} ms; media ${(elapsed/300).toFixed(3)} ms; p95 ${ordered[285].toFixed(3)} ms.`);expect(elapsed).toBeLessThan(15000);
},20000);
