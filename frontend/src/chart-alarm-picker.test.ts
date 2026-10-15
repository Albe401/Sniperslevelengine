import {it,expect,vi} from 'vitest';
import {ChartAlarmPicker} from './chart-alerts';
import type {ISeriesApi} from 'lightweight-charts';
function fixture(){
 const button={hidden:true,style:{top:'',right:''},textContent:'',title:'',setAttribute:vi.fn(),remove:vi.fn(),onclick:undefined as undefined|((e:any)=>void)};
 const details={open:false},input={value:'',focus:vi.fn()},listeners=new Map<string,(e:any)=>void>();
 const root={querySelector:(s:string)=>s==='.chart-alerts'?details:input};
 const guide={hidden:true,style:{top:'',right:''},append:vi.fn(),remove:vi.fn()},label={textContent:''};
 vi.stubGlobal('document',{createElement:(tag:string)=>tag==='button'?button:tag==='div'?guide:label});
 const host={dataset:{pricePaneHeight:undefined as string|undefined},clientHeight:500,append:vi.fn(),getBoundingClientRect:()=>({top:20}),closest:()=>root,addEventListener:(k:string,f:(e:any)=>void)=>listeners.set(k,f),removeEventListener:vi.fn()};
 const series={coordinateToPrice:vi.fn((y:number)=>200-y/10),priceScale:()=>({width:()=>80})};
 const picker=new ChartAlarmPicker(host as unknown as HTMLElement,series as unknown as ISeriesApi<'Candlestick'>);picker.bind('QQQ');
 return {guide,label,button,details,input,listeners,host,series,picker};
}
it('prefills the price selected by mouse and opens the alarm form without submitting',()=>{const f=fixture();f.listeners.get('pointermove')!({clientY:120,target:{}});expect(f.host.append).toHaveBeenCalledWith(f.guide,f.button);expect(f.button.style.right).toBe('84px');expect(f.label.textContent).toBe('190.00');expect(f.button.hidden).toBe(false);expect(f.button.textContent).toBe('+');expect(f.button.title).toBe('Crear alarma en $190.00');f.button.onclick!({preventDefault:vi.fn(),stopPropagation:vi.fn()});expect(f.details.open).toBe(true);expect(f.input.value).toBe('190.00');expect(f.input.focus).toHaveBeenCalledWith({preventScroll:true});});
it('rejects prices outside the plot and clears the shortcut on asset change',()=>{const f=fixture();f.listeners.get('pointermove')!({clientY:120,target:{}});f.picker.bind('AAPL');expect(f.button.hidden).toBe(true);f.button.onclick!({preventDefault:vi.fn(),stopPropagation:vi.fn()});expect(f.details.open).toBe(false);f.listeners.get('pointermove')!({clientY:519,target:{}});expect(f.button.hidden).toBe(true);});
it('keeps the selected price when the cursor enters the plus button and removes listeners on cleanup',()=>{const f=fixture();f.listeners.get('pointermove')!({clientY:120,target:{}});f.listeners.get('pointermove')!({clientY:150,target:f.button});f.button.onclick!({preventDefault:vi.fn(),stopPropagation:vi.fn()});expect(f.input.value).toBe('190.00');f.picker.dispose();expect(f.host.removeEventListener).toHaveBeenCalledTimes(2);expect(f.button.remove).toHaveBeenCalledOnce();expect(f.guide.remove).toHaveBeenCalledOnce();});

it('keeps alarm selection outside the CVD pane',()=>{const f=fixture();f.host.dataset.pricePaneHeight='300';f.listeners.get('pointermove')!({clientY:220,target:{}});expect(f.button.hidden).toBe(false);f.listeners.get('pointermove')!({clientY:370,target:{}});expect(f.button.hidden).toBe(true);});
