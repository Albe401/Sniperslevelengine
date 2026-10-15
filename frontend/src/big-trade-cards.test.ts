import {it,expect} from 'vitest';
import {bigTradeCards,recentBigTrades,BigTradeTape,bigTradeToolbar} from './big-trade-cards';
it('automatically removes expired cards and admits new prints without erasing the source',()=>{
 const tape=new BigTradeTape();const old={at:100000,shares:400,price:100,notional:40000,side:0,sequence:1};
 expect(tape.observe([old],'QQQ',159999)).toHaveLength(1);
 expect(tape.observe([old],'QQQ',160000)).toEqual([]);
 const next={...old,at:160000,sequence:2};expect(tape.observe([old,next],'QQQ',160000)).toEqual([next]);expect(old.at).toBe(100000);
 expect(tape.observe([old,next,{...next,at:220000,sequence:3}],'QQQ',220000)[0].sequence).toBe(3);
});
const p={at:Date.UTC(2026,9,9,17,35,11),shares:1500,price:750.82,notional:1126230,side:1,sequence:1};
it('shows exact shares and Eastern execution time, keeping details available without extra rows',()=>{
 const html=bigTradeCards([p],1);
 expect(html).toContain('1,500');expect(html).toContain('<time>13:35:11</time>');expect(html).toContain('Compra est.');expect(html).toContain('aria-pressed="true"');expect(html).toContain('$750.82');expect(html).toContain('lado inferido');
});
it('keeps unknown prints distinct from sellers and selects the individual sequence',()=>{
 const html=bigTradeCards([{...p,side:-1,sequence:2},{...p,side:0,sequence:3}],2);
 expect(html).toContain('Venta est.');expect(html).toContain('Sin clasificar');expect(html).toContain('data-trade-sequence="3"');
});
it('bounds the strip to newest five prints without mutating input or aggregating trades',()=>{
 const rows=Array.from({length:40},(_,i)=>({...p,at:p.at+i,sequence:i}));
 const latest=recentBigTrades(rows);expect(latest).toHaveLength(5);expect(latest[0].sequence).toBe(39);expect(latest.at(-1)?.sequence).toBe(35);expect(rows[0].sequence).toBe(0);
});
it('automatically replaces the oldest of five when a new print arrives',()=>{const tape=new BigTradeTape(),rows=Array.from({length:5},(_,i)=>({...p,at:p.at+i,sequence:i}));expect(tape.observe(rows,'QQQ',p.at+5)).toHaveLength(5);const next={...p,at:p.at+10,sequence:5};expect(tape.observe([...rows,next],'QQQ',p.at+10).map(t=>t.sequence)).toEqual([5,4,3,2,1]);});
it('preserves the strip node between live renders and removes unnecessary horizontal navigation',()=>{const html=bigTradeToolbar();expect(html).toContain('id="gl-big-trade-bar" data-chart-preserve');expect(html).not.toContain('data-trade-left');expect(html).not.toContain('data-trade-right');expect(html).not.toContain('data-trade-latest');expect(bigTradeCards([p],1)).toContain('class="gl-trade-price">$750.82');});
