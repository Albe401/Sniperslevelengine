import {it,expect} from 'vitest';
import {structuralQuality} from './structural-quality';
it('separates dominance from uncertain missing exposure',()=>{
 expect(structuralQuality(80,true,[75,85],90).label).toBe('Predominio CALL consistente');
 expect(structuralQuality(80,true,[7,80],16).label).toBe('Lectura incierta');
 expect(structuralQuality(20,true,[10,30],90).label).toBe('Predominio PUT consistente');
 expect(structuralQuality(49,false,null,null).label).toBe('Equilibrio consistente');
});
it('refuses missing or incoherent bounds and missing exposure',()=>{
 for(const b of [null,[81,90],[0,79],[90,10],[NaN,90],[-1,101]])expect(structuralQuality(80,true,b,90).consistent).toBe(false);
 expect(structuralQuality(null,false,[0,100],100).label).toBe('Datos insuficientes');
});
