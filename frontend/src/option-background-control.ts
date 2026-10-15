import {invoke,isTauri} from './snapshot-observation';
import {createRouteReader} from './route-live';
import type {LiveState} from './live';
const reader=createRouteReader(true);
let started=false,busy=false;
/** Runs regardless of view/selected asset; reuses the same formula with isolated state. */
export async function pollBackgroundOptionControl(){
 if(!isTauri()||busy)return;busy=true;
 try{
  const result=await invoke<{connected:boolean;frame?:LiveState}>('get_option_control_frame');
  if(result.connected&&result.frame?.symbol==='QQQ')reader.readRouteLive(result.frame,'QQQ',true);
  else reader.readRouteLive(undefined,'QQQ',false);
 }catch{reader.readRouteLive(undefined,'QQQ',false);}
 finally{busy=false;}
}
export function startBackgroundOptionControl(){
 if(started||!isTauri())return;started=true;
 void pollBackgroundOptionControl();setInterval(()=>void pollBackgroundOptionControl(),1000);
}
