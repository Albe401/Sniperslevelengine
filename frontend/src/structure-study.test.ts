import {describe,it,expect,vi} from 'vitest';
const mocks=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:mocks.invoke,isTauri:()=>true}));
// This handler test owns its catalog; a previously restored ten-asset snapshot
// must not determine this unit's expected inputs or mutate the product catalog.
vi.mock('./asset-watchlist',async(importOriginal)=>{
 const actual=await importOriginal<typeof import('./asset-watchlist')>();
 const symbols=['BRK.B',...Array.from({length:185},(_,i)=>'TEST'+i)];
 return {...actual,bundledAssets:Object.fromEntries(symbols.map(symbol=>[symbol,{name:symbol,kind:'STOCK',sector:null,live_range:1}]))};
});
import {wireStructureStudy,structureStudyPanel} from './structure-study';
describe('study queue start',()=>{
 it('sends the complete catalog including dotted symbols from the real start handler',async()=>{
  const button={dataset:{studyQueue:'start'},onclick:null as null|(()=>Promise<void>)};
  const root={querySelector:()=>null,querySelectorAll:(selector:string)=>selector==='[data-study-queue]'?[button]:[]} as unknown as Element;
  mocks.invoke.mockResolvedValue({queue:{remaining:['BRK.B'],status:'running',current:'BRK.B'},rows:[],running:true,paused:false});
  const paint=vi.fn();wireStructureStudy(root,paint);await button.onclick!();
  const args=mocks.invoke.mock.calls.at(-1)!;expect(args[0]).toBe('structure_queue');expect(args[1].action).toBe('start');expect(args[1].symbols).toContain('BRK.B');expect(args[1].symbols).toHaveLength(186);expect(paint).toHaveBeenCalled();
 });
 it('shows a rejected start next to the queue controls',async()=>{
  const button={dataset:{studyQueue:'start'},onclick:null as null|(()=>Promise<void>)};
  const root={querySelector:()=>null,querySelectorAll:(selector:string)=>selector==='[data-study-queue]'?[button]:[]} as unknown as Element;
  mocks.invoke.mockRejectedValue('TEST_START_REJECTED');wireStructureStudy(root,()=>{});await button.onclick!();
  const html=structureStudyPanel();expect(html.indexOf('TEST_START_REJECTED')).toBeLessThan(html.indexOf('data-study-queue="start"'));
 });
});
