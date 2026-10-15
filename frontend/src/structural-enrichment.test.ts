import {it,expect} from 'vitest';
import {scopeStructuralReport,structuralEvidenceReport,structuralEnrichmentPanel} from './structural-enrichment';
import type {EtfReport} from './etf-dealers';
import type {EtfDetail} from './etf-enrichment';
import type {StructuralCard} from './structural-map-preview';
import type {LiveState} from './live';
const now=Date.parse('2026-10-05T10:00:00-04:00');
const d=(expiration:string,kind:string,oi:number|null):EtfDetail=>({ticker:expiration+kind,expiration,kind,strike:500,oi,oiChange:oi==null?null:2,volume:3,volumeOi:.3,delta:.5,gamma:.01,dex:oi==null?null:100,gex:oi==null?null:200,iv:.2,theta:-.1,vega:.1,charm:.001,vanna:.002,bid:1,ask:1.1,spreadPct:10,quoteAt:now,realtime:true,fresh:true});
const r:EtfReport={symbol:'NVDA',days:28,at:now,spot:500,priceAt:now,expirations:['2026-10-05','2026-10-09'],contracts:2,gammaComplete:false,deltaComplete:false,rows:[],enrichment:{oiReferenceDate:null,previousObservedDate:'2026-10-02',oiComparable:true,priceFresh:true,oiKnown:1,gammaKnown:2,deltaKnown:2,quotesFresh:2,storageError:null,model:'Modelo BSM',details:[d('2026-10-05','call',10),d('2026-10-09','put',null)]}};
const card={name:'DIARIO',at:now,scopeReady:true,expirations:['2026-10-05']} as StructuralCard;
const s=()=>({symbol:'NVDA',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,structuralEnrichment:r}} as unknown as LiveState);
it('scopes concentration and quality to the real horizon dates and preserves unknown OI',()=>{
 const one=scopeStructuralReport(r,['2026-10-05']);expect(one.rows[0].call.oi).toBe(10);expect(one.rows[0].put.oi).toBe(0);expect(one.gammaComplete).toBe(true);expect(one.enrichment!.oiKnown).toBe(1);expect(one.contracts).toBe(1);
 const two=scopeStructuralReport(r,['2026-10-09']);expect(two.rows[0].put.oi).toBeNull();expect(two.gammaComplete).toBe(false);
});
it('rejects foreign stale future mismatched snapshots and invalid horizon dates',()=>{
 expect(structuralEvidenceReport(s(),'NVDA',true,card,now)?.contracts).toBe(1);
 expect(structuralEvidenceReport(s(),'SPY',true,card,now)).toBeUndefined();expect(structuralEvidenceReport(s(),'NVDA',false,card,now)).toBeUndefined();
 expect(structuralEvidenceReport(s(),'NVDA',true,card,now+300001)).toBeUndefined();expect(structuralEvidenceReport(s(),'NVDA',true,card,now-1)).toBeUndefined();
 expect(structuralEvidenceReport(s(),'NVDA',true,{...card,at:now-1},now)).toBeUndefined();expect(structuralEvidenceReport(s(),'NVDA',true,{...card,scopeReady:false},now)).toBeUndefined();
 expect(structuralEvidenceReport(s(),'NVDA',true,{...card,expirations:['2026-10-06']},now)).toBeUndefined();
});
it('renders rankings, selectable strikes and expiries, quality and local flow without replacing percentages',()=>{
 const html=structuralEnrichmentPanel('NVDA',s(),true,now,[card]);expect(html).toContain('data-structural-expiry');expect(html).toContain('data-structural-strike-select');expect(html).toContain('data-structural-strike="500"');expect(html).toContain('Calidad de cadena');expect(html).toContain('Gamma / GEX');expect(html).toContain('Flujo de opciones');expect(html).not.toContain('data-etf-chart');
});
