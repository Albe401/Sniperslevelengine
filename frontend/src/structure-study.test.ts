import {describe,it,expect,vi} from 'vitest';
const mocks=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:mocks.invoke,isTauri:()=>true}));
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
