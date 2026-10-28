export interface SavedFrameIdentity {available:boolean;id:string;capturedMs:number;frame:{symbol:string;publishedMs:number}}
export class FrameReadError extends Error {constructor(readonly code:string){super('Base de datos no disponible');}}
/** In-memory only, bounded to two assets. Revalidate with the exact server ETag;
 * cancellation and identity checks prevent late responses replacing another asset. */
export class WebFrameLoader<T extends SavedFrameIdentity> {
 private cache=new Map<string,{value:T;etag:string|null}>();
 private pending?:{symbol:string;controller:AbortController;promise:Promise<T>};
 constructor(private request:typeof fetch=(input,init)=>fetch(input,init),private clock=Date.now){}
 peek(symbol:string):T|undefined {return this.cache.get(symbol)?.value;}
 read(symbol:string):Promise<T>{
  if(this.pending?.symbol===symbol)return this.pending.promise;
  this.pending?.controller.abort();
  const controller=new AbortController();
  const promise=this.load(symbol,controller).finally(()=>{if(this.pending?.controller===controller)this.pending=undefined;});
  this.pending={symbol,controller,promise};return promise;
 }
 private async load(symbol:string,controller:AbortController):Promise<T>{
  const cached=this.cache.get(symbol),timer=setTimeout(()=>controller.abort(),60000);
  try{
   const response=await this.request(`/api/frame?symbol=${encodeURIComponent(symbol)}`,{cache:'no-store',signal:controller.signal,headers:cached?.etag?{'If-None-Match':cached.etag}:{}});
   if(controller.signal.aborted)throw new DOMException('Request superseded','AbortError');
   if(response.status===304){if(!cached)throw Error('Captura no disponible para revalidar');return cached.value;}
   if(!response.ok){const failure=await response.json().catch(()=>({}));throw new FrameReadError(typeof failure?.error==='string'?failure.error:'data_unavailable');}
   const value=await response.json() as T;
   if(controller.signal.aborted)throw new DOMException('Request superseded','AbortError');
   if(value?.available){
    if(value.frame?.symbol!==symbol||!Number.isSafeInteger(value.capturedMs)||value.capturedMs<=0||value.capturedMs>this.clock()||!Number.isSafeInteger(value.frame.publishedMs)||value.frame.publishedMs>value.capturedMs)throw Error('Lote inválido');
    this.cache.delete(symbol);this.cache.set(symbol,{value,etag:response.headers.get('ETag')});
    while(this.cache.size>2)this.cache.delete(this.cache.keys().next().value!);
   }
   return value;
  }finally{clearTimeout(timer);}
 }
}
