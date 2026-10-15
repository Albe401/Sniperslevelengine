import {invoke,isTauri} from './snapshot-observation';
import {sharedNeuroDirection} from './live';
export function neuroTraderPage():string {
  return `<section id="neuro-trader-workspace"><div class="panel-head"><div><span class="eyebrow">MÓDULO INDEPENDIENTE · MASSIVE</span><h1>Neuro Trader Engine</h1></div><button id="neuro-retry">Conectar módulo</button></div><p id="neuro-status" role="status">Iniciando módulo independiente…</p><iframe id="neuro-frame" title="Neuro Trader Engine" hidden style="width:100%;height:calc(100vh - 200px);min-height:680px;border:1px solid #332c6a;border-radius:12px;background:#07061a" sandbox="allow-scripts allow-same-origin allow-popups"></iframe></section>`;
}
export function bindNeuroTrader():void {
  const container=document.querySelector('#neuro-trader-workspace');if(!container)return;
  const status=container.querySelector<HTMLElement>('#neuro-status')!,frame=container.querySelector<HTMLIFrameElement>('#neuro-frame')!,button=container.querySelector<HTMLButtonElement>('#neuro-retry')!;
  let busy=false,last=0;
  const receive=async(event:MessageEvent)=>{
    if(!container.isConnected){window.removeEventListener('message',receive);return;}
    if(event.origin!=='http://127.0.0.1:8768'||event.source!==frame.contentWindow||event.data?.type!=='neuro-reading-v1'||!isTauri()||busy||Date.now()-last<4000)return;
    busy=true;last=Date.now();
    const id=event.data.id;
    try{
      const confirmation=await invoke<Record<string,unknown>>('neuro_confirmation',{reading:event.data.reading});
      const payload={...confirmation,direction:sharedNeuroDirection(event.data.reading.symbol)};
      if(container.isConnected)frame.contentWindow?.postMessage({type:'snipers-confirmation-v1',id,payload},'http://127.0.0.1:8768');
    }catch(error){if(container.isConnected)frame.contentWindow?.postMessage({type:'snipers-confirmation-v1',id,error:String(error)},'http://127.0.0.1:8768');}
    finally{busy=false;}
  };
  window.addEventListener('message',receive);
  const observer=new MutationObserver(()=>{if(!container.isConnected){window.removeEventListener('message',receive);observer.disconnect();}});
  observer.observe(container.parentNode!,{childList:true});
  const start=async()=>{
    if(!isTauri()){status.textContent='Abre Snipers para iniciar el módulo local.';return;}
    button.disabled=true;status.textContent='Iniciando Neuro Trader Engine…';
    try{const url=await invoke<string>('open_neuro_trader');if(!container.isConnected)return;
      if(url!=='http://127.0.0.1:8768/ui/')throw Error('Dirección del módulo no admitida');
      frame.src=`${url}?ui=${Date.now()}`;frame.hidden=false;status.textContent='Panel independiente conectado al servicio local. Consulta su estado de datos de Massive dentro del panel.';
    }catch(error){if(container.isConnected)status.textContent=String(error);}finally{button.disabled=false;}
  };
  button.onclick=()=>void start();void start();
}
