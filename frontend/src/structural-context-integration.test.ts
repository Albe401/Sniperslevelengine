import {afterEach,describe,expect,it,vi} from 'vitest';
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
import real from './structural-map-context.real.json';
import type {LiveState} from './live';
import type {PriceStructureCard} from './price-structure';
import * as priceEngine from './price-structure';
import * as evidence from './market-evidence';
import * as contextEngine from './structural-map-context';
import {priceStructureMap} from './price-structure-map';
import {massiveExposurePanel} from './massive-exposure-panel';
import {massiveExposureIdentity,readMassiveExposure} from './massive-exposure';
import {connectedStructureCards,structuralContextDetail} from './structural-context-detail';
import {structuralEnrichmentPanel} from './structural-enrichment';
import {escapeContextHtml} from './structural-context-html';
afterEach(()=>vi.restoreAllMocks());
function fixture(){return {s:structuredClone(real.state) as unknown as LiveState,cards:structuredClone(real.structure.cards) as PriceStructureCard[],now:real.now};}
function render(f:ReturnType<typeof fixture>,symbol='QQQ'){
 vi.spyOn(priceEngine,'priceStructureCards').mockReturnValue(f.cards);
 return priceStructureMap(symbol,f.s,true,f.now,[]);
}

describe('production structural map consumes the common context',()=>{
 it('connects the actual renderer and GEX table to one capture without recalculating cards',()=>{
  const f=fixture(),spy=vi.spyOn(contextEngine,'buildStructuralMapContext'),common=vi.spyOn(evidence,'readMarketEvidence');
  const html=render(f),gex=massiveExposurePanel(f.s,'QQQ',true,f.now,true),id=massiveExposureIdentity(readMassiveExposure(f.s,'QQQ',true,f.now)).replaceAll('"','&quot;');
  expect(spy).toHaveBeenCalledTimes(1);expect(common).toHaveBeenCalledTimes(1);expect(priceEngine.priceStructureCards).toHaveBeenCalledTimes(1);
  expect(html).toContain('data-structural-context="connected"');expect(html).toContain('data-horizon-details="structural-connections"');
  expect(gex).toContain(`data-options-context-id="${id}"`);expect(html.match(/data-options-context-id=/g)).toHaveLength(6);
  for(const m of html.matchAll(/data-options-context-id="([^"]+)"/g))expect(m[1]).toBe(id);
  expect(gex.match(/data-massive-exposure-strike=/g)).toHaveLength(76);expect(html.match(/data-price-structure-side=/g)).toHaveLength(5);expect(html.match(/data-price-summary/g)).toHaveLength(1);
  expect(html).toContain('103/128 contratos con gamma');expect(html).toContain('$704.88');expect(html).toContain('$704.39');expect(html).toContain('Subtotal parcial');expect(html).not.toContain('80% de probabilidad');
  const result=spy.mock.results[0].value as ReturnType<typeof contextEngine.buildStructuralMapContext>;expect(result.probability).toBeNull();expect(result.price.map(x=>x.side)).toEqual(real.expected.sides);
  if(process.env.SNIPERS_STRUCTURAL_CONNECTED_AUDIT_DIR){const dir=process.env.SNIPERS_STRUCTURAL_CONNECTED_AUDIT_DIR;writeFileSync(join(dir,'real-context-consumed.json'),JSON.stringify(result,null,2));writeFileSync(join(dir,'real-connected-map.html'),html);writeFileSync(join(dir,'real-connected-gex.html'),gex);}
 });
 it('keeps option failure separate from valid price structure',()=>{
  const f=fixture();f.s.optionSnapshotStatus.horizons[0].exposure.netGex+=1000;const html=render(f);
  expect(html).not.toContain('data-options-context-id=');expect(html).toContain('El subtotal no coincide');expect(html.match(/data-price-structure-side="PUT"/g)).toHaveLength(4);expect(html).toContain('data-price-structure-side="CALL"');
 });
 it('suppresses a cached foreign frame before rendering its directional cards',()=>{
  const f=fixture();f.s.symbol='SPY';const html=render(f);expect(html.match(/data-price-structure-side="SIN DATOS"/g)).toHaveLength(5);expect(html).not.toContain('data-options-context-id=');expect(html).not.toContain('$704.39');
 });
 it('suppresses a future close and retains the four other valid scales',()=>{
  const f=fixture();f.cards[0].endMs=f.now+1;const html=render(f);expect(html.match(/data-price-structure-side="SIN DATOS"/g)).toHaveLength(1);expect(html.match(/data-options-context-id=/g)).toHaveLength(5);expect(html).toContain('información futura');
 });
 it('replaces context IDs on a new capture and carries no previous-symbol cache',()=>{
  const f=fixture(),first=render(f),h=f.s.optionSnapshotStatus.horizons[0];h.observedMs--;f.s.optionSnapshotStatus.observedMs=h.observedMs;
  const next=priceStructureMap('QQQ',f.s,true,f.now,[]);expect(next).not.toBe(first);expect(next).toContain(massiveExposureIdentity(readMassiveExposure(f.s,'QQQ',true,f.now)).replaceAll('"','&quot;'));
  const missing=priceStructureMap('SPY',undefined,false,f.now,[]);expect(missing).not.toContain('data-options-context-id=');expect(missing).not.toContain('$704.39');
 });
});

describe('existing dialog and renderer boundaries',()=>{
 it('preserves the provider identity while escaping HTML characters',()=>{
  expect(escapeContextHtml('Massive-OI "<&\'')).toBe('Massive-OI &quot;&lt;&amp;&#39;');
 });
 it('does not reveal a card whose source envelope was not supplied',()=>{
  const f=fixture(),c=contextEngine.buildStructuralMapContext(f.s,'QQQ',true,f.now);expect(connectedStructureCards(f.cards,c).every(x=>x.side==='SIN DATOS')).toBe(true);
 });
 it('escapes source explanations and keeps five-minute activity scoped',()=>{
  const f=fixture(),c=contextEngine.buildStructuralMapContext(f.s,'QQQ',true,f.now);c.options.reason='<img src=x onerror=alert(1)>';c.options.profile=null;
  const html=structuralContextDetail(c);expect(html).not.toContain('<img');expect(html).toContain('&lt;img');expect(html).toContain('CVD de acciones');expect(html).toContain('5 minutos');expect(html).toContain('próximos cuatro vencimientos');expect(html).not.toContain('NEXT4');expect(c.activity.cvd.assignableStructureIntervals).toEqual([]);
 });
 it('rejects reused market evidence from a foreign session',()=>{
  const now=real.now,foreign={...evidence.readMarketEvidence(undefined,'SPY',false,now),symbol:'QQQ'},expected=structuralEnrichmentPanel('QQQ',undefined,false,now,[]);const spy=vi.spyOn(evidence,'readMarketEvidence');
  const html=structuralEnrichmentPanel('QQQ',undefined,false,now,[],'',foreign);expect(spy).toHaveBeenCalledTimes(1);expect(html).toBe(expected);
 });
 it('handles a null horizon preceding a valid Massive capture',()=>{
  const f=fixture();(f.s.optionSnapshotStatus.horizons as unknown[]).unshift(null);const c=contextEngine.buildStructuralMapContext(f.s,'QQQ',true,f.now);expect(c.options.profile?.rows).toHaveLength(76);
 });
});
