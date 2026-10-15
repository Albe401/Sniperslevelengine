import {it,expect} from 'vitest';
import {retainedContractProfile} from './last-contract-profile';
import type {LiveState} from './live';
const friday=Date.parse('2026-10-02T15:59:00-04:00'),sunday=Date.parse('2026-10-04T18:00:00-04:00'),opening=Date.parse('2026-10-05T09:30:00-04:00');
function fixture(symbol:string){return {symbol,optionSnapshotStatus:{structuralEnrichment:{symbol,at:friday,spot:190,expirations:['2026-10-05'],enrichment:{details:[{ticker:symbol+'call',expiration:'2026-10-05',strike:190,kind:'call',oi:10,volume:20,dex:100,gex:200}]}}},chartHistory:{sessions:[{date:'2026-10-05',openMs:opening}]},metrics:{phase:'Fuera de sesión'}} as unknown as LiveState;}
it('retains the actual Friday chain for weekend display and resets at Monday opening',()=>{
 const s=fixture('RETAIN1');expect(retainedContractProfile(s,s.symbol,sunday)?.rows[0].call.volume).toBe(20);
 expect(retainedContractProfile(s,s.symbol,opening-1)).toBeDefined();expect(retainedContractProfile(s,s.symbol,opening)).toBeUndefined();
 s.optionSnapshotStatus.structuralEnrichment!.at=opening;expect(retainedContractProfile(s,s.symbol,opening)?.at).toBe(opening);
});
it('rejects future, corrupt and foreign data without manufacturing contracts',()=>{
 const s=fixture('RETAIN2');s.optionSnapshotStatus.structuralEnrichment!.at=sunday+1;expect(retainedContractProfile(s,s.symbol,sunday)).toBeUndefined();
 s.optionSnapshotStatus.structuralEnrichment!.at=friday;s.optionSnapshotStatus.structuralEnrichment!.enrichment!.details[0].oi=-1;expect(retainedContractProfile(s,s.symbol,sunday)).toBeUndefined();
 expect(retainedContractProfile(s,'OTHER',sunday)).toBeUndefined();
});
it('historical horizon preserves expired Friday contracts on Sunday and after Monday opening',()=>{
 const s=fixture('HISTORY');const r=s.optionSnapshotStatus.structuralEnrichment!;
 r.expirations=['2026-10-02','2026-10-05'];
 r.enrichment!.details.push({...r.enrichment!.details[0],ticker:'HISTORYfriday',expiration:'2026-10-02',oi:20000});
 expect(retainedContractProfile(s,s.symbol,sunday,'DAY')?.rows[0].call.oi).toBe(20000);
 expect(retainedContractProfile(s,s.symbol,opening,'DAY')?.expirations).toEqual(['2026-10-02']);
 expect(retainedContractProfile(s,s.symbol,sunday,'NEXT4')?.rows[0].call.oi).toBe(20010);
 expect(retainedContractProfile(s,s.symbol,sunday,'WEEK')?.expirations).toEqual(['2026-10-02']);
});
it('Sunday snapshot exposes next-session DAY and WEEK contracts without changing snapshot date',()=>{
 const s=fixture('WEEKEND');const r=s.optionSnapshotStatus.structuralEnrichment!;r.at=sunday;
 r.enrichment!.details.push({...r.enrichment!.details[0],ticker:'WEEKENDtuesday',expiration:'2026-10-06',oi:15000});
 expect(retainedContractProfile(s,s.symbol,sunday,'DAY')?.rows[0].call.oi).toBe(10);
 expect(retainedContractProfile(s,s.symbol,sunday,'DAY')?.expirations).toEqual(['2026-10-05']);
 expect(retainedContractProfile(s,s.symbol,sunday,'WEEK')?.rows[0].call.oi).toBe(15010);
 expect(retainedContractProfile(s,s.symbol,sunday,'WEEK')?.at).toBe(sunday);
});
