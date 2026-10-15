import {describe,it,expect,vi} from 'vitest';
import {ChartAlerts,crossedAlert,validAlerts,type PriceAlert} from './chart-alerts';
import type {ISeriesApi} from 'lightweight-charts';
const a:PriceAlert={id:'a',price:100,direction:'either',sound:true};
const before={price:99,at:10000},after={price:101,at:11000};
describe('price crossing alarms',()=>{
 it('detects an upward cross and an exact touch',()=>{expect(crossedAlert(a,before,after,11000)).toBe(true);expect(crossedAlert(a,before,{...after,price:100},11000)).toBe(true);});
 it('detects downward crossings only for the requested direction',()=>{expect(crossedAlert({...a,direction:'down'},after,{price:98,at:12000},12000)).toBe(true);expect(crossedAlert({...a,direction:'up'},after,{price:98,at:12000},12000)).toBe(false);expect(crossedAlert({...a,direction:'down'},before,after,11000)).toBe(false);});
 it('does not alert on first observation or a price already beyond the level',()=>{expect(crossedAlert(a,undefined,after,11000)).toBe(false);expect(crossedAlert(a,after,{price:102,at:12000},12000)).toBe(false);});
 it('fires only once until rearmed',()=>expect(crossedAlert({...a,firedAt:9000},before,after,11000)).toBe(false));
 it('rejects repeated or reordered ticks',()=>{expect(crossedAlert(a,before,{...after,at:10000},11000)).toBe(false);expect(crossedAlert(a,before,{...after,at:9000},11000)).toBe(false);});
 it('rejects stale, future and disconnected gaps',()=>{expect(crossedAlert(a,before,after,30000)).toBe(false);expect(crossedAlert(a,before,after,9000)).toBe(false);expect(crossedAlert(a,before,{...after,at:30000},30000)).toBe(false);});
 it('rejects invalid price and clocks',()=>{expect(crossedAlert({...a,price:NaN},before,after,11000)).toBe(false);expect(crossedAlert(a,before,{...after,price:0},11000)).toBe(false);expect(crossedAlert(a,before,after,NaN)).toBe(false);});
 it('does not refire while the price remains at the threshold',()=>expect(crossedAlert(a,{price:100,at:10000},{price:100,at:11000},11000)).toBe(false));
 it('validates persisted data and removes duplicate IDs',()=>{expect(validAlerts([a,a,{...a,id:'b',price:-1},{...a,id:'c',direction:'sell'},{...a,id:'d',firedAt:-1}])).toEqual([a]);expect(validAlerts(null)).toEqual([]);});
 it('bounds the number of alarms per asset',()=>expect(validAlerts(Array.from({length:40},(_,i)=>({...a,id:String(i)})))).toHaveLength(30));
});
describe('chart alarm lifecycle with live samples',()=>{
 function fixture(symbol:string){
  const storage=new Map([[`snipers.alerts.v1.${symbol}`,JSON.stringify([a])]]);
  vi.stubGlobal('localStorage',{getItem:(k:string)=>storage.get(k)??null,setItem:(k:string,v:string)=>storage.set(k,v)});
  const notice={hidden:true,textContent:'',setAttribute(){this.hidden=true;}};
  const root={querySelector:(s:string)=>s==='.chart-alert-notice'?notice:null,querySelectorAll:()=>[]} as unknown as HTMLElement;
  const series={createPriceLine:vi.fn(()=>({})),removePriceLine:vi.fn()} as unknown as ISeriesApi<'Candlestick'>;
  return {storage,notice,root,series,engine:new ChartAlerts(series)};
 }
 it('persists a fired alert, shows the notice and removes its armed line',()=>{
  const f=fixture('ALERT1');f.engine.bind(f.root,'ALERT1',before,10000);expect(f.series.createPriceLine).toHaveBeenCalledOnce();
  f.engine.bind(f.root,'ALERT1',after,11000);expect(f.notice.hidden).toBe(false);expect(f.notice.textContent).toContain('ALERT1');expect(JSON.parse(f.storage.get('snipers.alerts.v1.ALERT1')!)[0].firedAt).toBe(11000);expect(f.series.removePriceLine).toHaveBeenCalledOnce();
  f.engine.bind(f.root,'ALERT1',{price:98,at:12000},12000);expect(f.series.removePriceLine).toHaveBeenCalledOnce();f.engine.dispose();
 });
 it('does not infer a crossing through a disconnect',()=>{
  const f=fixture('ALERT2');f.engine.bind(f.root,'ALERT2',before,10000);f.engine.bind(f.root,'ALERT2',undefined,10500);f.engine.bind(f.root,'ALERT2',after,11000);expect(f.notice.hidden).toBe(true);f.engine.dispose();
 });
 it('resets the baseline on asset switches',()=>{
  const f=fixture('ALERT3');f.engine.bind(f.root,'ALERT3',before,10000);f.engine.bind(f.root,'OTHER',{price:500,at:10500},10500);f.engine.bind(f.root,'ALERT3',after,11000);expect(f.notice.hidden).toBe(true);f.engine.dispose();
 });
});
