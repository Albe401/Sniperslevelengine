import {afterEach,describe,expect,it,vi} from 'vitest';
import real from './intraday-direction.real.json';
import snapshot from './structural-map-context.real.json';
import saved from './structural-context-web.saved.json';
import type {LiveState} from './live';
import type {PriceStructureCard} from './price-structure';
import type {StructuralIndicators} from './structural-indicators';
import {buildIntradayDirection,intradayDirectionLabel} from './intraday-direction';
import {priceStructureMap} from './price-structure-map';
import * as connected from './structural-context-detail';

afterEach(()=>vi.restoreAllMocks());
function input(){
 const card={...structuredClone(real.card),indicators:structuredClone(real.card.indicators) as StructuralIndicators};
 return {symbol:'QQQ',publishedMs:real.source.publishedMs,asOfMs:real.source.atMs,card:{...card,bars:card.indicators.bars}};
}

describe('saved 15m technical direction',()=>{
 it('preserves the real 11:00 QQQ CALL direction independently of MIXTA structure',()=>{
  const source=input(),before=JSON.stringify(source),r=buildIntradayDirection(source);
  expect(real.source.selection).toBe('cards[0]');expect(real.source.sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(real.source.capture.sha256).toMatch(/^[a-f0-9]{64}$/);expect(real.source.history.sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(Object.keys(real.source.kernelHashes)).toHaveLength(3);
  expect(real.source.atMs).toBe(1793026802471);expect(real.source.publishedMs).toBe(1793026800281);
  expect(r.availability).toBe('reference');expect(r.side).toBe('CALL');expect(r.structure).toBe('MIXTA');expect(r.relation).toBe('mixed');
  expect(r.closedAtMs).toBe(1793026800000);expect(r.asOfMs).toBe(real.source.atMs);expect(intradayDirectionLabel(r)).toBe('REF. CALL · 15m');
  expect(r.livePriceReady).toBe(false);expect(r.triggerConfirmed).toBe(false);expect(r.probability).toBeNull();expect(r.executionEnabled).toBe(false);
  expect(r.reason).toContain('histórica');expect(r.reason).toContain('No confirma una entrada.');expect(JSON.stringify(source)).toBe(before);
 });
 it.each(['CALL','PUT','MIXTA'] as const)('retains %s as a reference without computing another signal',side=>{
  // Synthetic feature variations verify the adapter protocol, not market performance.
  const f=input(),i=f.card.indicators;
  if(side==='PUT'){i.distanceAtr=-i.distanceAtr;i.slope3Atr=-i.slope3Atr;[i.plusDi,i.minusDi]=[i.minusDi,i.plusDi];}
  if(side==='MIXTA')i.slope3Atr=-i.slope3Atr;
  i.trend=side;const r=buildIntradayDirection(f);
  expect(r.side).toBe(side);expect(r.availability).toBe('reference');expect(intradayDirectionLabel(r)).toBe('REF. '+side+' · 15m');expect(r.livePriceReady).toBe(false);
 });
 it.each(['clock','future-close','published-after-capture','version','ema-seed','adx-seed','bars','not-ready','missing-indicators','missing-card','wrong-interval','missing-structure'] as const)('keeps %s pending',kind=>{
  const f=input();
  if(kind==='clock')f.card.indicators.at--;
  if(kind==='future-close'){f.card.endMs=f.asOfMs+1;f.card.indicators.at=f.card.endMs;}
  if(kind==='published-after-capture')f.publishedMs=f.asOfMs+1;
  if(kind==='version')(f.card.indicators as {version:string}).version='unrecognized';
  if(kind==='ema-seed')f.card.indicators.emaSeedAtr=.011;
  if(kind==='adx-seed')f.card.indicators.adxSeedPoints=.51;
  if(kind==='bars')f.card.bars--;
  if(kind==='not-ready'){f.card.indicators.ready=false;f.card.indicators.trend='PENDIENTE';f.card.indicators.strength='PENDIENTE';}
  if(kind==='missing-indicators')delete (f.card as {indicators?:unknown}).indicators;
  if(kind==='missing-card')delete (f as {card?:unknown}).card;
  if(kind==='wrong-interval')f.card.interval=60;
  if(kind==='missing-structure')f.card.side='SIN DATOS';
  const r=buildIntradayDirection(f);expect(r.availability).toBe('pending');expect(r.side).toBe('PENDIENTE');expect(r.closedAtMs).toBeNull();expect(r.livePriceReady).toBe(false);
 });
 it('never uses wall-clock freshness to make a saved reading current',()=>{
  const f=input();vi.spyOn(Date,'now').mockImplementation(()=>{throw new Error('wall clock forbidden');});
  expect(buildIntradayDirection(f).availability).toBe('reference');expect(intradayDirectionLabel(undefined)).toBe('REF. PENDIENTE · 15m');
 });
});

describe('existing snapshot map boundary',()=>{
 it('uses the connected card and keeps missing saved indicators neutral with the same six cards',()=>{
  const s={...structuredClone(snapshot.state),priceStructureReading:structuredClone(saved)} as unknown as LiveState;
  const joined=vi.spyOn(connected,'connectedStructureCards'),html=priceStructureMap('QQQ',s,true,snapshot.now,[]);
  expect(joined).toHaveBeenCalledTimes(1);expect(html).toContain('data-intraday-direction-status="pending"');expect(html).toContain('REF. PENDIENTE · 15m');
  expect(html).toContain('class="intraday-direction neutral"');expect(html.match(/data-price-structure-side=/g)).toHaveLength(5);expect(html.match(/data-price-summary/g)).toHaveLength(1);
  expect(html).toContain('data-horizon-details="structural-intraday-direction"');expect(html).toContain('referencia histórica');expect(html).not.toContain('data-intraday-direction-status="current"');
 });
 it('renders the real frozen direction from an accepted-card boundary without changing structure',()=>{
  // The boundary stub isolates presentation; upstream context validation has separate tests.
  const s={...structuredClone(snapshot.state),publishedMs:real.source.publishedMs,priceStructureReading:undefined} as unknown as LiveState;
  const cards=structuredClone(saved.cards) as PriceStructureCard[];Object.assign(cards[0],real.card,{bars:real.card.indicators.bars});
  vi.spyOn(connected,'connectedStructureCards').mockReturnValue(cards);
  const html=priceStructureMap('QQQ',s,true,real.source.atMs,[]);
  expect(html).toContain('REF. CALL · 15m');expect(html).toContain('data-intraday-direction-status="reference"');expect(html).toContain('data-price-structure-side="MIXTA"');
  expect(html).toContain('no acredita un precio actual');expect(html).not.toContain('data-intraday-direction-status="current"');expect(html.match(/data-price-structure-side=/g)).toHaveLength(5);
 });
});
