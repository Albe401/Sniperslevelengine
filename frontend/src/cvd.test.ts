import {expect,it} from 'vitest';
import {cvdPanel,type CvdRead} from './cvd';
import {analystTargetsPanel} from './analyst-targets';
const now=Date.parse('2026-10-02T14:00:00Z');
const p={buy:100,sell:20,unknown:10,delta:80,coveragePct:92.3,sinceMs:now-60000,lastMs:now,changeOne:10,changeFive:80,gaps:0};
const r:CvdRead={symbol:'NVDA',date:'2026-10-02',at:now,available:true,premarket:{...p},regular:{...p},rejected:0};
it('shows buyer pressure only with fresh matching evidence and sufficient coverage',()=>{
 expect(cvdPanel(r,'NVDA',true,'regular',now)).toContain('PRESIÓN COMPRADORA');
 for(const [read,symbol,time] of [[r,'AAPL',now],[r,'NVDA',now+16000],[{...r,available:false},'NVDA',now]] as const)expect(cvdPanel(read,symbol,true,'regular',time)).not.toContain('PRESIÓN COMPRADORA');
 expect(cvdPanel({...r,regular:{...p,coveragePct:30}},'NVDA',true,'regular',now)).toContain('CLASIFICACIÓN INSUFICIENTE');
});
it('keeps ambiguous pressure and malformed deltas from implying a direction',()=>{
 expect(cvdPanel({...r,regular:{...p,buy:100,sell:80,unknown:30,delta:20,coveragePct:85.7}},'NVDA',true,'regular',now)).toContain('SIN PREDOMINIO CONFIRMADO');
 expect(cvdPanel({...r,regular:{...p,delta:999}},'NVDA',true,'regular',now)).not.toContain('PRESIÓN COMPRADORA');
});
it('places the CVD below the price and above the three analyst targets',()=>{
 const html=analystTargetsPanel('NVDA',100,undefined,cvdPanel(r,'NVDA',true,'regular',now));
 expect(html.indexOf('Precio del activo')).toBeLessThan(html.indexOf('MOTOR CVD'));
 expect(html.indexOf('MOTOR CVD')).toBeLessThan(html.indexOf('LOW TARGET'));
});
