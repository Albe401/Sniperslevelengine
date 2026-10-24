import {it,expect} from 'vitest';
import {dayMap} from './day-map';
import type {Horizon} from './live';
import type {MapOutlook} from './day-map';
it('evaluates exact-level defense at the frozen capture clock and expires it only after that clock advances',()=>{
 const now=Date.parse('2026-10-12T15:00:00Z');
 const h={mode:'DAY',expirations:['2026-10-12'],structure:{complete:true,selected:{},putWall:{strike:100,score:80},routes:[]}} as unknown as Horizon;
 const outlook={rows:[],level:'',levelDetail:'',change:'',changeDetail:'',route:'',routeDetail:'',invalidate:'',invalidateDetail:'',marketEvidence:{levels:[],detail:'Fixture de reloj',at:now,price:{ready:true,spot:101},cvd:{signal:.5,coverage:90,at:now}},levelEvents:[{name:'PUT WALL',level:100,direction:1,action:'Defensa observada',at:now-120000,sustained:true}]} as unknown as MapOutlook;
 const render=(clock:number)=>dayMap(h,true,[],false,101,'',outlook,true,'',undefined,undefined,'',{},clock);
 expect(render(now)).toContain('Defensa compradora observada');
 expect(render(now+3001)).toContain('Defensa no verificable');
 expect(render(now+3001)).not.toContain('Defensa compradora observada');
});
