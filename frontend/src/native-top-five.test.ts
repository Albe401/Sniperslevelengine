import {it,expect,afterEach} from 'vitest';
import {nativePriority,nativeMetrics,rankNative,setNativeHorizon,shortlistNative,nativeReports,nativeOptionsPanel} from './native-top-five';
import type {EtfReport} from './etf-dealers';
const now=Date.parse('2026-10-02T15:00:00Z');
const c=(symbol:string,volume=100)=>({symbol,volume,price:100,changePct:1,volumeMetrics:{rvolPremarket:2}});
const r=(symbol:string):EtfReport=>({symbol,days:7,at:now,spot:100,priceAt:now,contracts:2,expirations:['2026-10-02'],gammaComplete:true,deltaComplete:true,rows:[{strike:100,call:{oi:100,volume:100,delta:5000,gamma:25000},put:{oi:100,volume:50,delta:4000,gamma:25000}}],enrichment:{priceFresh:true,oiKnown:2,gammaKnown:2,deltaKnown:2,quotesFresh:2,details:[],oiReferenceDate:null,previousObservedDate:null,oiComparable:false,storageError:null,model:'native'}});
afterEach(()=>{setNativeHorizon(7);for(const s of Object.keys(nativeReports))delete nativeReports[s];});
it('reranks beyond the original top five using native option activity',()=>{
 const rows=Array.from({length:10},(_,i)=>c('A'+i,100-i));
 expect(rankNative(rows,now,{A9:r('A9')})[0].symbol).toBe('A9');
 expect(rankNative(rows,now,{A9:r('A9')})).toHaveLength(5);
});
it('does not certify missing, partial, expired, wrong-asset or wrong-horizon data',()=>{
 for(const bad of [undefined,{...r('A'),at:now+1},{...r('B')},{...r('A'),days:0},{...r('A'),gammaComplete:false},{...r('A'),at:now-300001},{...r('A'),rows:[]},{...r('A'),enrichment:null}])expect(nativePriority(c('A'),bad,now)).toBeNull();
 const missing=r('A');missing.rows[0].put.oi=null;expect(nativePriority(c('A'),missing,now)).toBeNull();
});
it('preserves units and never assumes zero for unknown metrics',()=>{
 const report=r('A'),m=nativeMetrics(report);expect(m.oi).toBe(200);expect(m.volume).toBe(150);expect(m.gex).toBe(50000);expect(m.dex).toBe(9000);expect(m.volumeOi).toBe(.75);expect(m.nearShare).toBe(1);
 report.rows[0].call.gamma=null;expect(nativeMetrics(report).gex).toBeNull();
});
it('caps requests at twenty unique valid candidates with deterministic ties',()=>{
 expect(shortlistNative(Array.from({length:100},(_,i)=>c('A'+i,i+1)))).toHaveLength(20);
 expect(shortlistNative([c('D'),c('D'),c('N',NaN),c('B'),c('A')]).map(x=>x.symbol)).toEqual(['A','B']);
});
it('clears snapshots when switching horizon and labels priority as an index',()=>{
 nativeReports.A=r('A');expect(nativeOptionsPanel(c('A'),now)).toContain('/100');
 setNativeHorizon(0);expect(nativeReports.A).toBeUndefined();expect(nativeOptionsPanel(c('A'),now)).toContain('0DTE');
 expect(nativeOptionsPanel(c('A'),now)).not.toContain('Neuro');
});
it('permits explicitly provisional high coverage but excludes low coverage',()=>{
 const report=r('A');report.gammaComplete=false;
 const detail={ticker:'O:A',expiration:'2026-10-02',strike:100,kind:'call',oi:190,oiChange:null,volume:100,volumeOi:null,delta:.5,gamma:.01,dex:9500,gex:47500,iv:.2,theta:null,vega:null,charm:null,vanna:null,bid:1,ask:1.1,spreadPct:10,quoteAt:now,realtime:true,fresh:true};
 report.rows[0].call.gamma=null;report.enrichment!.details=[detail];
 expect(nativeMetrics(report).gex).toBeNull();expect(nativeMetrics(report).coverage).toBe(.95);
 expect(nativePriority(c('A'),report,now)).not.toBeNull();
 nativeReports.A=report;expect(nativeOptionsPanel(c('A'),now)).toContain('provisional');
 detail.oi=100;expect(nativePriority(c('A'),report,now)).toBeNull();
});
it('benchmarks twenty candidates with 2048 strikes each',()=>{
 const rows=Array.from({length:20},(_,i)=>c('A'+i)),reports:Record<string,EtfReport>={};
 for(const candidate of rows){const report=r(candidate.symbol);report.rows=Array.from({length:2048},(_,i)=>({...report.rows[0],strike:50+i*.05}));reports[candidate.symbol]=report;}
 const start=performance.now();for(let i=0;i<50;i++)rankNative(rows,now,reports);
 const mean=(performance.now()-start)/50;console.info(`TOP5_NATIVE: 20 candidatos x 2048 strikes, ${mean.toFixed(2)} ms por clasificación`);expect(mean).toBeLessThan(100);
});
it('shows options as information with an old price without assigning priority',()=>{
 const report=r('A');report.enrichment!.priceFresh=false;nativeReports.A=report;
 expect(nativePriority(c('A'),report,now)).toBeNull();
 const html=nativeOptionsPanel(c('A'),now);expect(html).toContain('snapshot informativo');expect(html).toContain('100 / 100');expect(html).toContain('precio antiguo: no clasifica');
});
it('explicitly reports an empty 0DTE chain instead of substituting a later expiry',()=>{
 setNativeHorizon(0);nativeReports.A={...r('A'),days:0,rows:[],expirations:[],contracts:0};
 expect(nativeOptionsPanel(c('A'),now)).toContain('Sin contratos 0DTE');expect(nativePriority(c('A'),nativeReports.A,now)).toBeNull();
});
