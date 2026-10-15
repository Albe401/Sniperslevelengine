import {expect,it} from 'vitest';
import {levelVolume,dayMap,levelStrength} from './day-map';
import type {ContractWindow} from './flow-coverage';
import type {Horizon} from './live';
it('rechaza puntuaciones inválidas sin confundir cero con ausencia',()=>{
 for(const score of [undefined,null,NaN,Infinity,-1,101])expect(levelStrength(score)).toContain('Sin datos');
 expect(levelStrength(0)).toContain('0.0/100');
 expect(levelStrength(82)).toContain('82.0/100');
 expect(levelStrength(82,true,true)).not.toContain('PROVISIONAL'); expect(levelStrength(82,true,true)).toContain('cadena incompleta');
});
it('muestra fuerza de cada imán cuando el motor entrega puntuaciones completas',()=>{
 const structure={complete:true,selected:{M1:745},magnet:740,mainMagnetScore:73,liquidityMagnetScore:66,callWall:{strike:745,score:82},putWall:{strike:735,score:61},routes:[{direction:'CALL',vacuumLow:746,vacuumHigh:748,vacuumScore:91}]};
 const h={mode:'DAY',expirations:[],structure} as unknown as Horizon;
 const html=dayMap(h,true,[],false,null);
 expect(html).toContain('82.0/100');expect(html).toContain('61.0/100');expect(html).toContain('91.0/100');expect(html).toContain('73.0/100');expect(html).toContain('66.0/100');
 expect(html.match(/aria-valuenow="82"/g)).toHaveLength(1);
 expect(html).toContain('Sin datos');
 expect(dayMap(h,false,[],false,null)).not.toContain('82.0/100');
 h.structure.provisional={...h.structure,mainMagnetScore:null,liquidityMagnetScore:null,coveredContracts:10,totalContracts:20,callWall:{strike:750,score:43,ready:false}};
 const partial=dayMap(h,true,[],false,null);
 expect(partial).toContain('43.0/100');expect(partial).not.toContain('82.0/100');expect(partial).not.toContain('73.0/100');expect(partial).not.toContain('66.0/100');expect(partial).not.toContain('PROVISIONAL');expect(partial).toContain('Cobertura de cadena');
});
it('muestra límites y estado del vacío sin inventar volumen total',()=>{
 const h={mode:'DAY',expirations:['2026-09-25'],structure:{complete:true,reading:{available:false},selected:{},routes:[{direction:'CALL',vacuumLow:741,vacuumHigh:743,vacuumStatus:'VACUUM'},{direction:'PUT',vacuumLow:730,vacuumHigh:732,vacuumStatus:'LOW-LIQUIDITY POCKET'}]}} as unknown as Horizon;
 const html=dayMap(h,true,[],false,null);
 expect(html).toContain('741.00 — 743.00');expect(html).toContain('730.00 — 732.00');
 expect(html).toContain('Vacío superior');expect(html).toContain('Vacío inferior'); expect(html).toContain('Bolsillo de baja liquidez · sin confirmar');
 expect(html).toContain('no es volumen total de la zona');
 expect(dayMap(h,false,[],false,null)).not.toContain('741.00');
});
it('limita el mapa a muros, imanes y vacíos aunque existan niveles secundarios',()=>{
 const h={mode:'DAY',expirations:['2026-09-25'],structure:{complete:true,reading:{available:false},selected:{R1:745,R2:750,R3:755,S1:735,S2:730,S3:725,M1:740,M2:738},callWall:{strike:745},putWall:{strike:735},magnet:740,routes:[{direction:'CALL',target:750,extended:760,barrier:748}]}} as unknown as Horizon;
 const html=dayMap(h,true,[],false,null);
 for(const value of ['755.00','725.00','738.00','760.00','748.00'])expect(html).not.toContain(value);
 expect(html.split('<strong>745.00</strong>')).toHaveLength(2);
 expect(html).toContain('CALL WALL');expect(html).not.toContain('<span>R1</span>');expect(html).not.toContain('Objetivo');expect(html).not.toContain('Barrera');expect(html.match(/<article class="day-map-node /g)).toHaveLength(6);
});
const rows:ContractWindow[]=['call','put'].map((kind,i)=>({ticker:kind,kind,strike:740,expiration:'2026-09-25',minutes:1,covered:true,contracts:i?4:6,premium:null,trades:1,reason:''}));
it('adds real barriers and nests the exact pull target in its wall only in the approved route design',()=>{
 const h={mode:'DAY',expirations:[],structure:{complete:true,selected:{},callWall:{strike:745,score:82},putWall:{strike:730,score:61},routes:[{direction:'CALL',barrier:746.5,barrierScore:68,pullTarget:744,score:72},{direction:'PUT',barrier:728.5,barrierScore:65,pullTarget:731.5,score:69}]}} as unknown as Horizon;
 const html=dayMap(h,true,[],false,null,'',undefined,true);
 expect(html).toContain('BARRERA DE LIQUIDEZ UP');expect(html).toContain('746.50');expect(html).toContain('CALL PULL <b>744.00</b>');expect(html).toContain('PUT PULL <b>731.50</b>');
 expect(dayMap(h,false,[],false,null,'',undefined,true)).not.toContain('744.00');
 expect(dayMap(h,true,[],false,null)).not.toContain('CALL PULL');
});
it('suma los dos lados del strike y vencimiento exactos',()=>{expect(levelVolume([...rows,{...rows[0],strike:741,contracts:999}],740,['2026-09-25'],1,true)).toBe(10);});
it('no confunde cobertura incompleta, caducada o de otra ventana con cero',()=>{expect(levelVolume(rows.slice(0,1),740,['2026-09-25'],1,true)).toBeNull();expect(levelVolume(rows,740,['2026-09-25'],1,false)).toBeNull();expect(levelVolume(rows,740,['2026-09-25'],5,true)).toBeNull();expect(levelVolume(rows,740,['2026-09-26'],1,true)).toBeNull();});
it('conserva cero observado y rechaza duplicados',()=>{expect(levelVolume(rows.map(r=>({...r,contracts:0})),740,['2026-09-25'],1,true)).toBe(0);expect(levelVolume([...rows,rows[0]],740,['2026-09-25'],1,true)).toBeNull();});
it('no inventa niveles sin estructura',()=>{expect(dayMap(undefined,false,rows,true,null)).toContain('Esperando estructura');expect(dayMap(undefined,false,rows,true,null)).not.toContain('740.00');});
it('mantiene ambos cuadros de vacíos sin rangos y no duplica el que tiene datos',()=>{
 const empty=dayMap(undefined,false,[],false,null);
 expect(empty).toContain('CALL WALL');
 expect(empty).toContain('Vacío superior');expect(empty).toContain('Vacío inferior');
 expect(empty.match(/day-map-node vacuum/g)).toHaveLength(2);
 const h={mode:'DAY',expirations:[],structure:{complete:true,reading:{available:false},selected:{},routes:[{direction:'CALL',vacuumLow:741,vacuumHigh:743,vacuumStatus:'VACUUM'}]}} as unknown as Horizon;
 const partial=dayMap(h,true,[],false,null);
 expect(partial.match(/day-map-node vacuum/g)).toHaveLength(2);
 expect(partial).toContain('741.00 — 743.00');
 expect(partial).toContain('Sin rango disponible en la estructura del día');
});
it.each([
 {direction:'CALL',label:'Vacío superior',wall:'CALL WALL',low:747,high:749,above:true},
 {direction:'CALL',label:'Vacío superior',wall:'CALL WALL',low:741,high:743,above:false},
 {direction:'PUT',label:'Vacío inferior',wall:'PUT WALL',low:737,high:739,above:true},
 {direction:'PUT',label:'Vacío inferior',wall:'PUT WALL',low:731,high:733,above:false},
])('sitúa $label ($low–$high) por precio respecto a $wall',({direction,label,wall,low,high,above})=>{
 const h={mode:'DAY',expirations:[],structure:{complete:true,reading:{available:false},selected:{},callWall:{strike:745},putWall:{strike:735},routes:[{direction,vacuumLow:low,vacuumHigh:high,vacuumStatus:'VACUUM'}]}} as unknown as Horizon;
 const html=dayMap(h,true,[],false,null);
 expect(html.indexOf(`<span>${label}</span>`) < html.indexOf(`<span>${wall}</span>`)).toBe(above);
 // A later structure update moves the same card across the wall.
 h.structure.routes[0].vacuumLow=above?720:760;
 h.structure.routes[0].vacuumHigh=above?722:762;
 const updated=dayMap(h,true,[],false,null);
 expect(updated.indexOf(`<span>${label}</span>`) < updated.indexOf(`<span>${wall}</span>`)).toBe(!above);
});
it('ordena los seis cuadros sin datos y distingue los dos imanes con datos',()=>{
 const labels=['CALL WALL','Vacío superior','IMÁN PRINCIPAL','IMÁN DE LIQUIDEZ','Vacío inferior','PUT WALL'];
 const html=dayMap(undefined,false,[],false,null);
 const positions=labels.map(label=>html.indexOf(`<span>${label}</span>`));
 expect(positions.every(n=>n>=0)).toBe(true);
 expect(positions).toEqual([...positions].sort((a,b)=>a-b));
 const h={mode:'DAY',expirations:[],structure:{complete:true,reading:{available:false},selected:{M1:740},magnet:738,routes:[]}} as unknown as Horizon;
 const live=dayMap(h,true,[],false,null);
 expect(live).toContain('IMÁN PRINCIPAL</span><strong>740.00');
 expect(live).toContain('IMÁN DE LIQUIDEZ</span><strong>738.00');
});
