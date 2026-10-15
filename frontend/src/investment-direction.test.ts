import {expect,it} from 'vitest';
import {directionOrder,frameScore,investmentDirectionPanel} from './investment-direction';
import type {AssetConfig} from './asset-watchlist';
const asset=(kind='STOCK'):AssetConfig=>({kind,name:'Company',sector:null,live_range:5});
it('puts QQQ SPY first, then mega cap, ETFs and the remaining companies',()=>{
 const registry={ZZZ:asset(),SPY:asset('ETF'),QQQ:asset('ETF'),MSFT:asset(),NVDA:asset(),IWM:asset('ETF'),AAA:asset(),SPX:asset('INDEX')};
 expect(directionOrder(registry,{ZZZ:500,AAA:200}).map(([s])=>s)).toEqual(['QQQ','SPY','NVDA','MSFT','IWM','ZZZ','AAA']);
});
it('preserves unknown companies without pretending to know market cap',()=>{expect(directionOrder({ZZZ:asset(),AAA:asset()}).map(([s])=>s)).toEqual(['AAA','ZZZ']);});
it('does not turn absent per-symbol evidence into neutral or a borrowed percentage',()=>{
 const result=frameScore('SPY',[],true,Date.now());expect(result.side).toBe('PENDIENTE');expect(result.call).toBeNull();
});
it('keeps the directional module separate from investment fundamentals',()=>{
 const html=investmentDirectionPanel();expect(html).toContain('Compañías para CALL y PUT');expect(html).toContain('data-id-side');expect(html).toContain('data-id-min');expect(html).toContain('No arma el bot');expect(html.indexOf('<b>QQQ</b>')).toBeLessThan(html.indexOf('<b>SPY</b>'));
});
