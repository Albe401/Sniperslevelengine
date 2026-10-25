import {invoke as nativeInvoke,isTauri,type InvokeArgs,type InvokeOptions} from '@tauri-apps/api/core';
import {rememberGexInventory} from './gex-windows';
import {snapshotCommandAllowed,snapshotSymbols} from './snapshot-model';
export * from '@tauri-apps/api/core';
const last=new Map<string,number>();
let observationsEnabled=false;
export async function invoke<T>(command:string,args?:InvokeArgs,options?:InvokeOptions):Promise<T>{
 const value=await (arguments.length===1?nativeInvoke<T>(command):arguments.length===2?nativeInvoke<T>(command,args):nativeInvoke<T>(command,args,options));
 if(command==='get_contract_inventory')rememberGexInventory(value);
 if(observationsEnabled&&typeof window!=='undefined'&&'__TAURI_INTERNALS__' in window&&snapshotCommandAllowed(command)){
  const key=JSON.stringify([command,args]);const now=Date.now();
  if(now-(last.get(key)??0)>=300000){
   last.set(key,now);if(last.size>256)last.delete(last.keys().next().value!);
   void nativeInvoke('record_snapshot_section',{command,args:args??{},value}).catch(()=>{});
  }
 }
 return value;
}
let collecting=false,lastCollection=0;
export function startSnapshotSectionCollection(){
 if(!isTauri())return;
 const collect=async()=>{
  if(collecting||Date.now()-lastCollection<300000)return;collecting=true;
  try{
   try{await nativeInvoke('get_snapshot_status');observationsEnabled=true;}catch{observationsEnabled=false;return;}
   lastCollection=Date.now();
   // All ten assets, independent of selected page. Reuse existing quantitative readers.
   for(const symbol of snapshotSymbols){
    await Promise.allSettled([
     invoke('get_contract_inventory',{symbol})
    ]);
    await new Promise(resolve=>setTimeout(resolve,1500));
   }
   await Promise.allSettled([invoke('get_fmp_targets')]);
  }finally{collecting=false;}
 };
 void collect();setInterval(()=>{void collect();},30000);
}
