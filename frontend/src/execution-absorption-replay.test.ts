import {expect,it} from 'vitest';
import replay from './execution-absorption-replay.fixture.json';
import {readExecutionAbsorption} from './execution-absorption-reading';
import {cvdPanel,type CvdRead} from './cvd';
import {chartBigTrades} from './big-trades';
import {bigTradeCards} from './big-trade-cards';
import {absorptionOverlaySvg} from './execution-absorption-overlay';
it.each(['buyer','seller'] as const)('consumes actual Rust-serialized %s replay through CVD, tape and chart',side=>{
 const read=replay[side] as unknown as CvdRead,view=readExecutionAbsorption(read,'QQQ',true,replay.now);
 expect(view.status).toBe('possible');expect(view.candidates).toHaveLength(1);
 expect(view.candidates[0].absorbingSide).toBe(side==='buyer'?'buy':'sell');
 const cvd=cvdPanel(read,'QQQ',true,'regular',replay.now);
 expect(cvd).toContain(side==='buyer'?'PRESIÓN VENDEDORA':'PRESIÓN COMPRADORA');
 expect(cvd).toContain(side==='buyer'?'Abs. compradora':'Abs. vendedora');
 const trades=chartBigTrades(read,'QQQ',100000,replay.now);expect(trades).toHaveLength(10);
 expect(bigTradeCards(trades,undefined,view.candidates)).toContain(side==='buyer'?'Venta est. · Abs. compradora':'Compra est. · Abs. vendedora');
 const svg=absorptionOverlaySvg(view,()=>({x:200,y:120}),700,400);
 expect(svg).toContain(side==='buyer'?'ABS. COMPRA EST.':'ABS. VENTA EST.');
});
it.each(['breakthrough','disconnected','noDepth'] as const)('never draws an absorption signal for Rust-serialized %s evidence',key=>{
 const read=replay[key] as unknown as CvdRead,view=readExecutionAbsorption(read,'QQQ',true,replay.now);
 expect(view.status).not.toBe('possible');expect(view.candidates).toEqual([]);
 expect(absorptionOverlaySvg(view,()=>({x:200,y:120}),700,400)).toBe('');
});
it('expires a real serialized reading and retains synthetic provenance',()=>{expect(readExecutionAbsorption(replay.buyer as unknown as CvdRead,'QQQ',true,replay.now+3001).status).toBe('pending');expect(replay.basis).toContain('synthetic replay');});
