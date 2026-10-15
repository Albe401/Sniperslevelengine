import {it,expect} from 'vitest';
import {etfProfileSvg,usableEtfReport,etfDealersPage,type EtfReport} from './etf-dealers';
import {scrollStrikeCenter} from './strike-profile-scroll';
const report:EtfReport={symbol:'SPY',days:28,at:1000,spot:500,priceAt:900,expirations:['2026-10-05'],contracts:240,gammaComplete:true,deltaComplete:true,rows:Array.from({length:120},(_,i)=>({strike:450+i,call:{oi:i,volume:2*i,delta:i*3,gamma:i*4},put:{oi:i+1,volume:3*i,delta:i*4,gamma:i*5}}))};
it('reaches strikes outside the former top ten in both directions',()=>{
 const strikes=report.rows.map(r=>r.strike);
 let center:number|undefined;
 for(let i=0;i<30;i++)center=scrollStrikeCenter(strikes,center,500,-320);
 expect(center).toBe(569);expect(etfProfileSvg(report,'oi',center,569)).toContain('569.00');
 for(let i=0;i<30;i++)center=scrollStrikeCenter(strikes,center,500,320);
 expect(center).toBe(450);expect(etfProfileSvg(report,'oi',center,450)).toContain('450.00');
});
it('does not accept another symbol, horizon, stale report or duplicate strike',()=>{
 expect(usableEtfReport(report,'SPY',28,1000)).toBe(true);
 expect(usableEtfReport(report,'QQQ',28,1000)).toBe(false);
 expect(usableEtfReport(report,'SPY',0,1000)).toBe(false);
 expect(usableEtfReport(report,'SPY',28,301001)).toBe(false);
 expect(usableEtfReport({...report,rows:[report.rows[0],report.rows[0]]},'SPY',28,1000)).toBe(false);
 const midnight=Date.parse('2026-10-05T00:00:00-04:00');expect(usableEtfReport({...report,at:midnight-1000},'SPY',28,midnight+1000)).toBe(false);
});
it('renders missing metrics as unknown and preserves real zero',()=>{
 const r={...report,rows:[{strike:500,call:{oi:0,volume:null,delta:null,gamma:null},put:{oi:null,volume:null,delta:null,gamma:null}}]};
 expect(etfProfileSvg(r,'oi',500,500)).toContain('PUT —');
 expect(etfProfileSvg(r,'oi',500,500)).toContain('CALL 0');
 expect(etfProfileSvg(r,'gamma',500,500)).not.toContain('NaN');
});
it('shows SPY and QQQ profiles side by side without a price chart or symbol tabs',()=>{
 const html=etfDealersPage();expect(html).toContain('class="etf-profiles"');expect(html).toContain('data-etf-profile="SPY"');expect(html).toContain('data-etf-profile="QQQ"');expect(html.match(/data-etf-profile-wheel/g)).toHaveLength(2);expect(html).not.toContain('data-etf-chart');expect(html).not.toContain('data-etf-symbol');expect(html).toContain('4 semanas');expect(html).not.toContain('Neuro');expect(html).not.toContain('Top 10');
});

it('keeps the components module exclusive to the QQQ scanner panel',()=>{const html=etfDealersPage(),spy=html.slice(html.indexOf('data-etf-profile="SPY"'),html.indexOf('data-etf-profile="QQQ"')),qqq=html.slice(html.indexOf('data-etf-profile="QQQ"'));expect(spy).not.toContain('data-qqq-components');expect(qqq).toContain('data-qqq-components');expect(html.match(/data-qqq-components/g)).toHaveLength(1);});
