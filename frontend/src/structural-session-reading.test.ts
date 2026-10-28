import {describe,expect,it} from 'vitest';
import type {PriceStructureCard} from './price-structure';
import {structureDisplayState,structuralSessionReading,type SessionDirectionEvidence} from './structural-session-reading';
const now=Date.parse('2026-10-27T11:00:00-04:00');
const card=(side:PriceStructureCard['side']='CALL',interval=15)=>({interval,side,endMs:now,bars:120,atr:2,lastClose:720,seedDifference:.001,highs:[],lows:[],reference:false}as unknown as PriceStructureCard);
const direction=()=>({symbol:'QQQ',side:'CALL',availability:'current',closedAtMs:now,livePriceReady:true,triggerConfirmed:false,probability:null,executionEnabled:false,reason:'No confirma una entrada.'}as SessionDirectionEvidence);
describe('intraday bias remains distinct from context and entry',()=>{
 it('preserves an existing verified CALL against opposing larger scales without changing its input',()=>{
  const cards=[card(),card('PUT',60),card('PUT',240),card('PUT',1440),card('PUT',10080)],d=direction(),before=JSON.stringify({cards,d});
  expect(structuralSessionReading('QQQ',cards,d,now)).toMatchObject({side:'CALL',availability:'current',livePriceReady:true});
  expect(JSON.stringify({cards,d})).toBe(before);expect(d.triggerConfirmed).toBe(false);expect(d.executionEnabled).toBe(false);
 });
 it.each(['foreign-symbol','different-close','future-close','duplicate-scale','missing-scale','pending','rejected'] as const)('does not borrow a direction for %s',kind=>{
  let cards=[card()];const d=direction();
  if(kind==='foreign-symbol')d.symbol='SPY';if(kind==='different-close')cards[0].endMs!--;
  if(kind==='future-close'){d.closedAtMs!++;cards[0].endMs=d.closedAtMs;}
  if(kind==='duplicate-scale')cards.push(card());if(kind==='missing-scale')cards=[];
  if(kind==='pending'||kind==='rejected')d.availability=kind;
  expect(structuralSessionReading('QQQ',cards,d,now)).toMatchObject({side:'PENDIENTE',availability:'pending',livePriceReady:false});
 });
 it('keeps a historical result explicitly historical and a pending live price pending',()=>{
  const d=direction();d.availability='reference';expect(structuralSessionReading('QQQ',[card()],d,now)).toMatchObject({side:'CALL',availability:'reference',livePriceReady:false});
  d.availability='current';d.livePriceReady=false;expect(structuralSessionReading('QQQ',[card()],d,now)).toMatchObject({availability:'current',livePriceReady:false});
 });
 it('distinguishes missing history, warmup and measured candles still waiting for pivots without promoting the engine state',()=>{
  const c=card('SIN DATOS');expect(structureDisplayState(c)).toBe('FORMANDO');expect(c.side).toBe('SIN DATOS');
  c.bars=40;expect(structureDisplayState(c)).toBe('HISTORIAL');
  c.bars=0;c.atr=null;expect(structureDisplayState(c)).toBe('SIN DATOS');
  c.bars=120;c.atr=2;c.seedDifference=.1;expect(structureDisplayState(c)).toBe('SIN VERIFICAR');
 });
});
