import {it,expect} from 'vitest';
import {optionSideVolume,callPutVolumePanel} from './call-put-volume';
import type {ContractWindow} from './flow-coverage';
const rows:ContractWindow[]=[{ticker:'CALL1',kind:'CALL',strike:100,expiration:'2026-09-24',minutes:1,covered:true,contracts:30,premium:null,trades:1,reason:''},{ticker:'PUT1',kind:'PUT',strike:100,expiration:'2026-09-24',minutes:1,covered:true,contracts:10,premium:null,trades:1,reason:''}];
it('separates CALL/PUT and only counts the requested expirations and window',()=>{
 expect(optionSideVolume(rows,['2026-09-24'],'CALL',1,true)).toBe(30);
 expect(optionSideVolume(rows,['2026-09-24'],'PUT',1,true)).toBe(10);
 expect(optionSideVolume(rows,['2026-09-24'],'CALL',5,true)).toBeNull();
 expect(callPutVolumePanel(rows,['2026-09-24'],true)).toContain('CALL 75.0 %');
});
it('missing, stale, duplicate or incomplete coverage never becomes zero',()=>{
 expect(optionSideVolume(rows,['2026-09-24'],'CALL',1,false)).toBeNull();
 expect(optionSideVolume(rows,['2026-09-25'],'CALL',1,true)).toBeNull();
 expect(optionSideVolume([...rows,rows[0]],['2026-09-24'],'CALL',1,true)).toBeNull();
 expect(optionSideVolume([{...rows[0],covered:false}],['2026-09-24'],'CALL',1,true)).toBeNull();
 expect(optionSideVolume([{...rows[0],contracts:0}],['2026-09-24'],'CALL',1,true)).toBe(0);
});
