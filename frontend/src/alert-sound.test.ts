import {it,expect,vi,afterEach} from 'vitest';
import {validAlertSoundSettings,saveAlertSoundSettings,readAlertSoundSettings,alertSoundNotes,enableAlertSound,playAlertSound} from './alert-sound';
afterEach(()=>vi.unstubAllGlobals());
it('defaults invalid preferences and retains a saved sound across reloads',()=>{
 expect(validAlertSoundSettings({tone:'unknown',volume:NaN})).toEqual({tone:'classic',volume:60});
 const store=new Map<string,string>();vi.stubGlobal('localStorage',{setItem:(k:string,v:string)=>store.set(k,v),getItem:(k:string)=>store.get(k)??null});
 expect(saveAlertSoundSettings({tone:'bell',volume:60})).toBe(true);expect(readAlertSoundSettings().tone).toBe('bell');
});
it('retains the in-memory choice when storage fails',()=>{
 vi.stubGlobal('localStorage',{setItem:()=>{throw new Error('unavailable');}});expect(saveAlertSoundSettings({tone:'double',volume:60})).toBe(false);
 expect(alertSoundNotes('double')).toHaveLength(2);expect(alertSoundNotes('urgent')).toHaveLength(3);
});
it('uses the selected pattern for actual audio and waits for user activation',async()=>{
 const starts:number[]=[];
 class Context {
  state='suspended';currentTime=10;destination={};async resume(){this.state='running';}
  createOscillator(){return {type:'',frequency:{value:0},connect:vi.fn(),disconnect:vi.fn(),onended:null,start:(at:number)=>starts.push(at),stop:vi.fn()};}
  createGain(){return {connect:vi.fn(),disconnect:vi.fn(),gain:{setValueAtTime:vi.fn(),linearRampToValueAtTime:vi.fn(),exponentialRampToValueAtTime:vi.fn()}};}
 }
 vi.stubGlobal('AudioContext',Context);saveAlertSoundSettings({tone:'urgent',volume:60});expect(playAlertSound()).toBe(false);
 expect(await enableAlertSound()).toBe(true);expect(playAlertSound()).toBe(true);expect(starts).toEqual([10,10.2,10.4]);
 starts.length=0;expect(playAlertSound('bell')).toBe(true);expect(starts).toEqual([10,10.16]);
});
