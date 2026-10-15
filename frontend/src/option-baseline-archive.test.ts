import {describe,it,expect} from 'vitest';
import {OptionBaselineArchive,type BaselinePacket} from './option-baseline-archive';
const packet=(at=10000):BaselinePacket=>({version:'option-formula-baseline-v1',source:'desktop-live',symbol:'QQQ',session:'2026-10-12',observedMs:at,publishedMs:at-1,connection:1,confluence:{version:'direction-confluence-40-40-20-v1',call:60,put:40,side:'CALL',components:[]},quant:{version:'quant-route-close-first-10m-v1',horizonMinutes:10,state:'waiting',at,probabilities:null,sampleId:'',side:0,upper:null,lower:null}});
describe('formula baseline archive',()=>{
 it('freezes outputs despite later mutations',()=>{const a=new OptionBaselineArchive();const p=packet();a.observe(p);p.confluence.call=99;expect(a.export().rows[0].confluence.call).toBe(60);const x=a.export();x.rows[0].confluence.call=1;expect(a.export().rows[0].confluence.call).toBe(60);});
 it('rejects future and stale sources and duplicate seconds',()=>{const a=new OptionBaselineArchive();a.observe({...packet(),publishedMs:11000});a.observe({...packet(),publishedMs:0});a.observe(packet());a.observe(packet(10999));expect(a.export().rows).toHaveLength(1);});
 it('preserves pending probability and handles unavailable persistence',()=>{const a=new OptionBaselineArchive({getItem:()=>null,setItem:()=>{throw Error('quota');}});a.observe(packet());expect(a.export().rows[0].quant.probabilities).toBeNull();expect(a.export().storageError).toContain('memoria');});
});
