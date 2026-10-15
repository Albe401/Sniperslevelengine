import {it,expect} from 'vitest';
import {alphaPath,momentumAlpha} from './momentum-alpha';
const at=600000;
const bars=()=>Array.from({length:8},(_,i)=>({timestampMs:at-(8-i)*60000,open:100,high:100.2,low:99.8,close:100,volume:100}));
it('blocks a PUT before a known bounce and mirrors CALL rejection',()=>{const b=bars();b[3]={...b[3],low:99.4};expect(alphaPath(b,at,100,-1,97,101,1).clear).toBe(false);expect(alphaPath(b,at,100,-1,97,101,1).obstacle).toBe(99.4);const c=b.map(x=>({...x,open:200-x.open,close:200-x.close,high:200-x.low,low:200-x.high}));expect(alphaPath(c,at,100,1,103,99,1).clear).toBe(false);});
it('does not use future candles to identify a support',()=>{const b=bars();b.push({timestampMs:at,open:100,high:100,low:98,close:99,volume:100});expect(alphaPath(b,at,100,-1,97,101,1)).toEqual(alphaPath(b.slice(0,-1),at,100,-1,97,101,1));});
it('missing or discontinuous candles never prove a clean path',()=>{expect(alphaPath([],at,100,-1,97,101,1).ready).toBe(false);const b=bars();b.splice(3,1);expect(alphaPath(b,at,100,-1,97,101,1).ready).toBe(false);});
it('a blocked path cannot activate even with maximal momentum',()=>{const path={ready:true,clear:false,obstacle:99.4,space:.5,reason:'Rebote'};expect(momentumAlpha({score:100,state:'ACTIVO',acceleration:.2},path,2,1,2,1).state).toBe('CAMINO BLOQUEADO');});
it('requires comparable volume growth and preserves missing RVOL',()=>{const p={ready:true,clear:true,obstacle:null,space:3,reason:'Disponible'},m={score:100,state:'ACTIVO',acceleration:.2};expect(momentumAlpha(m,p,null,null,2,1).state).toBe('ACTIVO');expect(momentumAlpha(m,p,null,null,2,1).rvolChange).toBeNull();expect(momentumAlpha(m,p,null,null,null,null).score).toBeNull();expect(momentumAlpha(m,p,1,2,2,1).state).toBe('PREPARÁNDOSE');});
