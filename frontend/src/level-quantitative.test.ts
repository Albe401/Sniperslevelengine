import {it,expect} from 'vitest';
import {quantitativeLevel,levelDefense} from './level-quantitative';
import type {MarketEvidence} from './market-evidence';
import type {LevelEvent} from './intraday-map';
it('requires sustained exact-level price reaction and fresh aligned covered CVD',()=>{
 const now=Date.UTC(2026,9,12,15),m={at:now,price:{ready:true,spot:101},cvd:{signal:.5,coverage:90,at:now}} as MarketEvidence;
 const e:LevelEvent={name:'PUT WALL',level:100,direction:1,action:'Defensa observada',at:now-120000,sustained:true};
 expect(levelDefense(100,m,[e],now).label).toBe('Defensa compradora observada');
 expect(levelDefense(100,{...m,price:{...m.price,spot:99},cvd:{...m.cvd,signal:-.5}},[{...e,direction:-1,action:'Rechazo observado'}],now).label).toBe('Defensa vendedora observada');
 expect(levelDefense(100,{...m,cvd:{...m.cvd,signal:-.5}},[e],now).label).toBe('Defensa contradicha');
 for(const event of [{...e,sustained:false},{...e,level:101},{...e,at:now+1},{...e,at:now-300001},{...e,action:'Pérdida sostenida'}])expect(levelDefense(100,m,[event],now).label).toBe('Defensa pendiente');
 for(const c of [{...m.cvd,coverage:79},{...m.cvd,at:now-3001},{...m.cvd,signal:NaN},{...m.cvd,at:now+1}])expect(levelDefense(100,{...m,cvd:c},[e],now).label).toBe('Defensa pendiente');
 expect(levelDefense(100,undefined,[e],now).label).toBe('Defensa pendiente');
});
import type {BidAskWindow} from './strike-bid-ask';
const date='2026-10-12';
const rows:BidAskWindow[]=[{ticker:'c',kind:'call',strike:100,expiration:date,covered:true,ask:80,bid:20,unknown:0},{ticker:'p',kind:'put',strike:100,expiration:date,covered:true,ask:20,bid:80,unknown:0}];
it('infers pressure symmetrically without assigning all option purchases to CALL',()=>{
 const r=quantitativeLevel(100,undefined,rows,[date],true);expect(r.imbalance).toBe(.6);expect(r.bullish).toBe(160);expect(r.probability).toBeNull();
 expect(quantitativeLevel(100,undefined,rows.map(r=>({...r,ask:r.bid,bid:r.ask})),[date],true).imbalance).toBe(-.6);
});
it('rejects stale, absent sides, duplicates, foreign strikes and insufficient classification',()=>{
 for(const rs of [rows.slice(0,1),[...rows,rows[0]],rows.map(r=>({...r,unknown:100})),rows.map(r=>({...r,covered:false})),rows.map(r=>({...r,strike:101}))])expect(quantitativeLevel(100,undefined,rs,[date],true).imbalance).toBeNull();
 expect(quantitativeLevel(100,undefined,rows,[date],false).imbalance).toBeNull();
});
it('preserves neutral observed pressure and leaves zero activity undefined',()=>{
 expect(quantitativeLevel(100,undefined,rows.map(r=>({...r,ask:50,bid:50})),[date],true).imbalance).toBe(0);
 expect(quantitativeLevel(100,undefined,rows.map(r=>({...r,ask:0,bid:0})),[date],true).imbalance).toBeNull();
});
