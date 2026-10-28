import {it,expect} from 'vitest';
import {dmi14,structuralIndicators} from './structural-indicators';
import {resetStructuralIndicatorCache} from './structural-technical-cache';
import type {PriceBar} from './price-structure';

// Protocol data isolates the equality branch; market-data acceptance is recorded separately.
const symmetric=(downExtra=0):PriceBar[]=>Array.from({length:120},(_,i)=>({
 timestampMs:1793107800000+i*900000,endMs:1793108700000+i*900000,sessionDate:'2026-10-27',
 open:731.15,close:731.15,high:(73170+29*i)/100,low:(73060-29*i)/100-i*downExtra,volume:100
}));
it('does not create directional movement from equal decimal high/low expansions',()=>{
 expect(dmi14(symmetric())).toEqual({plusDi:0,minusDi:0,adx:0});
});
it('keeps exact zero DMI and ADX through cache warmup, appends and corrections',()=>{
 resetStructuralIndicatorCache();const bars=symmetric();
 for(const size of [100,110,120,120,105,120]){
  const read=structuralIndicators(bars.slice(0,size),15,1,'decimal-tie');
  expect([read.plusDi,read.minusDi,read.adx]).toEqual([0,0,0]);
 }
 const revised=bars.map(b=>({...b}));revised[100].high+=.01;
 const cached=structuralIndicators(revised,15,1,'decimal-tie'),direct=dmi14(revised);
 expect(cached.plusDi).toBeCloseTo(direct.plusDi!,12);
 expect(cached.minusDi).toBeCloseTo(direct.minusDi!,12);
 expect(cached.adx).toBeCloseTo(direct.adx!,12);
});
it('preserves a small real directional difference above binary price resolution',()=>{
 const direct=dmi14(symmetric(1e-8));
 expect(direct.plusDi).toBe(0);expect(direct.minusDi).toBeGreaterThan(0);expect(direct.adx).toBeCloseTo(100,10);
 resetStructuralIndicatorCache();const cached=structuralIndicators(symmetric(1e-8),15,1,'real-small-difference');
 expect(cached.minusDi).toBeCloseTo(direct.minusDi!,12);expect(cached.adx).toBeCloseTo(100,10);
});
