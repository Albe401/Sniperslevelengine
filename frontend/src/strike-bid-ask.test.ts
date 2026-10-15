import {describe,it,expect} from 'vitest';
import {strikeBidAsk,bidAskSvg,type BidAskWindow} from './strike-bid-ask';
const row:BidAskWindow={ticker:'A',kind:'call',strike:100,expiration:'2026-10-05',covered:true,bid:15,ask:20,unknown:3};
describe('strike bid ask',()=>{
 it('scopes expiration and side without substituting missing puts or zero DTE',()=>{
  const rs=strikeBidAsk([row,{...row,ticker:'B',kind:'put',expiration:'2026-10-09'}],['2026-10-05'],[100],true);
  expect(rs[0].callFlow).toEqual({bid:15,ask:20,unknown:3,series:1});expect(rs[0].put).toBeNull();
  expect(strikeBidAsk([row],[],[100],true)[0].call).toBeNull();
 });
 it('revokes disconnected, incomplete, duplicate and invalid samples',()=>{
  for(const rows of [[{...row,covered:false}],[{...row,bid:-1}],[row,row]])expect(strikeBidAsk(rows,['2026-10-05'],[100],true)[0].call).toBeNull();
  expect(strikeBidAsk([row],['2026-10-05'],[100],false)[0].call).toBeNull();
 });
 it('renders quantities, delta and unclassified sample explicitly',()=>{
  const rs=strikeBidAsk([row],['2026-10-05'],[100],true);
  const svg=bidAskSvg(rs.map(r=>({...r,y:200})),600,120);
  expect(svg).toContain('Δ 5');expect(svg).toContain('sin clasificar 3');expect(svg).toContain('A 20 / B 15');expect(svg).toContain('5 min');
 });
});
