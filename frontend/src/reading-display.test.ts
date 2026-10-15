import {expect,it} from 'vitest';
import {mapHorizon,offSession} from './reading-display';
import {dayMap} from './day-map';
import {horizonCards} from './visual-readings';
import type {Horizon,OffSummary} from './live';
const h={mode:'NEXT_SESSION',label:'Próximo vencimiento',expirations:['2026-09-28'],exposure:{spot:745,gexComplete:false},structure:{complete:false,reading:{available:false},routes:[],selected:{},provisional:{coveredContracts:800,totalContracts:1020,selected:{M1:745},callWall:{strike:755},putWall:{strike:735},magnet:744,routes:[]}}} as unknown as Horizon;
it('permite próximo vencimiento explícito sin sustituirlo por NEXT4',()=>{
 expect(mapHorizon([h])).toBe(h);
 const day={...h,mode:'DAY'};expect(mapHorizon([h,day])).toBe(day);
 expect(mapHorizon([{...h,mode:'NEXT4'}])).toBeUndefined();
});
it('muestra referencias parciales etiquetadas y no puntuaciones confirmadas',()=>{
 const html=dayMap(h,true,[],true,null);
 expect(html).toContain('755.00');expect(html).toContain('800/1020');expect(html).toContain('Próximo vencimiento');
 expect(dayMap(h,false,[],true,null)).not.toContain('755.00');
 const asset=horizonCards([h],true);
 expect(asset).toContain('755.00');expect(asset).toContain('Referencias provisionales');expect(asset).toContain('800/1020');
 expect(horizonCards([h],false)).not.toContain('755.00');
});
const session=(date:string,print_count:number)=>({date,print_count}) as OffSummary;
it('recupera última sesión con prints y respeta selección explícita incluso vacía',()=>{
 const today=session('2026-09-26',0),friday=session('2026-09-25',200),thursday=session('2026-09-24',300);
 expect(offSession(today,[thursday,friday])).toBe(friday);
 expect(offSession(today,[friday],today.date)).toBe(today);
 expect(offSession(today,[friday],thursday.date)).toBe(friday);
 expect(offSession({...today,print_count:1},[friday])?.date).toBe(today.date);
 expect(offSession(today,[])).toBe(today);expect(offSession(undefined)).toBeUndefined();
});
