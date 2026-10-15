import {it,expect} from 'vitest';
import {biasRouteEvidence,biasRouteMap} from './bias-route-map';
import type {ControlReading} from './intraday-control';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,2,15);
const reading=()=>({symbol:'QQQ',priceNow:{ready:true,spot:742.5,direction:1,headline:'PRECIO SUBIENDO',pace:'Avance'},flowControl:{quality:'ready',tone:'buyers',headline:'COMPRADORES'},strikeReading:{rows:[]},intradayMap:{levels:[{name:'IMÁN PRINCIPAL',level:742}],events:[{name:'IMÁN PRINCIPAL',level:742,direction:1,sustained:true,action:'Defensa observada',at:now-120000}]}} as unknown as ControlReading);
const state=()=>({symbol:'QQQ',publishedMs:now,options:{phase:'streaming',lastEventMs:now},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-10-02'],structure:{complete:true,selected:{M1:742},callWall:{strike:745},putWall:{strike:730},magnet:742,routes:[{direction:'CALL',barrier:744,pullTarget:750}]}}]},flow:{intraday_windows:[{minutes:5,event_count:10,premium_complete:true,buckets:Object.fromEntries(['CALL BUY','PUT BUY','CALL SELL','PUT SELL','UNKNOWN'].map(k=>[k,{premium:1000}]))}]}} as unknown as LiveState);
it('audit: complete bullish evidence produces only a candidate, never a final CALL',()=>{
 const s=state(),r=reading(),e=biasRouteEvidence(s,r,true,now);
 expect(e.premium).toBe(true);expect(e.comparisonReady).toBe(true);expect(e.candidate).toBe(1);expect(e.confirmation).toBe(false);
 const html=biasRouteMap(s,r,true,now);
 expect(html).toContain('CAMBIO HACIA CALL · EN EVALUACIÓN');
 expect(html).toMatch(/data-bias-node="call"[^>]*>[\s\S]*?<strong>PENDIENTE<\/strong>/);
 expect(html).toContain('Todavía no se confirma');
});
it('audit: bearish evidence selects the nearest lower level symmetrically',()=>{
 const r=reading();r.priceNow!.direction=-1;r.priceNow!.headline='PRECIO CAYENDO';r.flowControl!.tone='sellers';r.flowControl!.headline='VENDEDORES';r.intradayMap!.events[0].direction=-1;
 const e=biasRouteEvidence(state(),r,true,now);
 expect(e.candidate).toBe(-1);expect(e.next?.level).toBe(742);expect(e.confirmation).toBe(false);
});
it.each(['publication','event-expired','event-future','event-unsustained','price','partial-flow','opposed-price'])('audit: blocks unsupported candidates: %s',kind=>{
 const s=state(),r=reading();
 if(kind==='publication')s.publishedMs=now-3001;
 if(kind==='event-expired')r.intradayMap!.events[0].at=now-180001;
 if(kind==='event-future')r.intradayMap!.events[0].at=now+1;
 if(kind==='event-unsustained')r.intradayMap!.events[0].sustained=false;
 if(kind==='price')r.priceNow!.ready=false;
 if(kind==='partial-flow')r.flowControl!.quality='partial';
 if(kind==='opposed-price')r.priceNow!.direction=-1;
 expect(biasRouteEvidence(s,r,true,now).candidate).toBe(0);
});
it('audit: possible absorption requires contrary flow and current RVOL and does not elect a final side',()=>{
 const r=reading();r.flowControl!.tone='sellers';r.flowControl!.headline='VENDEDORES';r.participation={rvol:1.2,at:now};
 const e=biasRouteEvidence(state(),r,true,now);
 expect(e.absorption.kind).toBe('fall');expect(e.candidate).toBe(0);expect(e.confirmation).toBe(false);
 r.participation.at=now-180001;
 expect(biasRouteEvidence(state(),r,true,now).absorption.kind).toBe('waiting');
});
it('audit: snapshot delta is displayed but does not decide the candidate',()=>{
 const r=reading(),before=biasRouteEvidence(state(),r,true,now);
 r.strikeReading={rows:[{name:'CALL WALL',strike:745,expiration:'2026-10-02',call:.01,put:-.99,at:now,change:null,strength:null,pressure:'Pendiente',partial:false}]};
 const after=biasRouteEvidence(state(),r,true,now);
 expect(after.deltaRows).toHaveLength(1);expect(after.candidate).toBe(before.candidate);expect(after.confirmation).toBe(false);
});
it('audit: confirmation remains pending over successive fresh observations',()=>{
 for(let i=0;i<12;i++){
  const t=now+i*2000,s=state();s.publishedMs=t;s.options.lastEventMs=t;
  const e=biasRouteEvidence(s,reading(),true,t);
  expect(e.candidate).toBe(1);expect(e.confirmation).toBe(false);
 }
});
it('audit: rising price may show a conditional level even when option comparison is missing',()=>{
 const s=state(),r=reading();s.flow.intraday_windows=[];r.flowControl!.quality='waiting';r.flowControl!.tone='waiting';r.flowControl!.headline='CONTROL SIN CONFIRMACIÓN';
 const e=biasRouteEvidence(s,r,true,now);
 expect(e.premiumObserved).toBe(false);expect(e.comparisonReady).toBe(false);expect(e.candidate).toBe(0);expect(e.next?.level).toBe(744);
 expect(biasRouteMap(s,r,true,now)).toContain('No garantiza alcanzarla');
});
it('uses the nearest real obstacle ahead of price, not the furthest target',()=>{const e=biasRouteEvidence(state(),reading(),true,now);expect(e.next?.level).toBe(744);expect(e.confirmation).toBe(false);});
it('keeps a pullback from electing the opposite side',()=>{const r=reading();r.priceNow!.headline='RETROCESO DURANTE LA SUBIDA';const e=biasRouteEvidence(state(),r,true,now);expect(e.counter).toBe(true);expect(e.candidate).toBe(0);expect(biasRouteMap(state(),r,true,now)).toContain('SIN GIRO CONFIRMADO');});
it('slowing is not a reversal confirmation',()=>{const r=reading();r.priceNow!.pace='PERDIENDO VELOCIDAD';const e=biasRouteEvidence(state(),r,true,now);expect(e.slowing).toBe(true);expect(e.confirmation).toBe(false);});
it.each(['disconnect','foreign','stale','future','partial','negative'])('withholds invalid premium: %s',kind=>{const s=state();if(kind==='foreign')s.symbol='SPY';if(kind==='stale')s.options.lastEventMs=now-15001;if(kind==='future')s.options.lastEventMs=now+1;if(kind==='partial')s.flow.intraday_windows![0].premium_complete=false;if(kind==='negative')s.flow.intraday_windows![0].buckets!['CALL BUY'].premium=-1;expect(biasRouteEvidence(s,reading(),kind!=='disconnect',now).premium).toBe(false);});
it('does not fill missing trade data with the current snapshot delta',()=>{const r=reading();r.strikeReading={rows:[{name:'CALL WALL',strike:745,expiration:'2026-10-02',call:.6,put:-.4,at:now,change:null,strength:null,pressure:'Pendiente',partial:false}]};const e=biasRouteEvidence(state(),r,true,now);expect(e.deltaRows).toHaveLength(1);expect(e.confirmation).toBe(false);});
it('renders connected boxes, arrows, targets and escaped symbol',()=>{const r=reading();r.symbol='<img>';const html=biasRouteMap(undefined,r,false,now);expect(html).toContain('&lt;img&gt;');expect(html).toContain('bias-route-arrow');expect(html).toContain('¿HASTA DÓNDE');expect(html).toContain('¿SE AGOTA');expect(html).not.toContain('has-evidence');});
it('shows partial observed premium without authorizing a candidate',()=>{const s=state();s.flow.intraday_windows![0].premium_complete=false;const e=biasRouteEvidence(s,reading(),true,now);expect(e.premiumObserved).toBe(true);expect(e.callBuy).not.toBe('—');expect(e.candidate).toBe(0);expect(biasRouteMap(s,reading(),true,now)).toContain('Actividad parcial');});
it.each(['foreign','previous','stale','future'])('rejects invalid block context: %s',kind=>{const s=state();const p={count:4,total:30000000,buy:20000000,sell:10000000,unknown:0,coverage_pct:100,complete:true,through_ms:now,top_levels:[]};s.offExchange={analytics:{blocks:{symbol:kind==='foreign'?'SPY':'QQQ',date:kind==='previous'?'2026-10-01':'2026-10-02',regular:{...p,through_ms:kind==='stale'?now-15001:kind==='future'?now+1:now},premarket:p}}} as unknown as LiveState['offExchange'];expect(biasRouteEvidence(s,reading(),true,now).regular).toBe('Sin evidencia vigente');});
it('keeps session blocks separate from options premium and confirmation',()=>{const s=state();const p={count:4,total:30000000,buy:20000000,sell:10000000,unknown:0,coverage_pct:100,complete:true,through_ms:now,top_levels:[]};s.offExchange={analytics:{blocks:{symbol:'QQQ',date:'2026-10-02',regular:p,premarket:p}}} as unknown as LiveState['offExchange'];const e=biasRouteEvidence(s,reading(),true,now);expect(e.regular).toContain('$20M');expect(e.confirmation).toBe(false);expect(e.callBuy).toBe('$1K');});
