import type {EtfReport} from './etf-dealers';
import {buildOiHeatmap,fromMassiveReference,fromMassiveSnapshots,underlyingFromMassiveSnapshots,type Heatmap} from './option-oi-heatmap';
/** Uses the independent reference retained from the native inventory REST pages. */
export function reportOiHeatmap(r:EtfReport,mode:'column'|'matrix'='column',colorScale:'linear'|'log1p'='linear'):Heatmap {
 const source=r.heatmap;
 if(!source||!Array.isArray(source.reference)||!Array.isArray(source.snapshots)||typeof source.referenceComplete!=='boolean')throw Error('Referencia independiente no disponible; actualiza la cadena.');
 const dates=new Set(r.expirations);
 const expected=fromMassiveReference(source.reference),observations=fromMassiveSnapshots(source.snapshots);
 // Invalid dates remain visible to validation instead of disappearing in scope filters.
 const scopedTickers=new Set(expected.filter(c=>dates.has(c.expiration)).map(c=>c.ticker));
 const inScope=(c:{ticker:string;expiration:string})=>scopedTickers.has(c.ticker)||!/^\d{4}-\d{2}-\d{2}$/.test(c.expiration)||dates.has(c.expiration);
 return buildOiHeatmap({symbol:r.symbol,observedMs:r.at,oiDate:r.enrichment?.oiReferenceDate??null,populationId:`${r.symbol}:${source.scopeStart}:${source.scopeEnd}:${r.at}:${[...dates].sort().join(',')}`,referenceComplete:source.referenceComplete,
  expected:expected.filter(inScope),observations:observations.filter(inScope),normalization:mode,scale:colorScale,maxUnderlyingAgeMs:120_000,
  underlying:underlyingFromMassiveSnapshots(source.snapshots,r.symbol,r.at,120_000)});
}

