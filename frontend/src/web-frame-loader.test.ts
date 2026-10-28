import {it,expect,vi}from'vitest';
import {WebFrameLoader,FrameReadError}from'./web-frame-loader';
const at=Date.UTC(2026,9,28,19),frame=(symbol='QQQ',id='one')=>({available:true,id,capturedMs:at-1000,frame:{symbol,publishedMs:at-1001,allStrikes:[500,695,700,900]}});
it('revalidates without downloading or replacing an unchanged captured frame',async()=>{
 const request=vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(frame()),{headers:{ETag:'W/"one"'}})).mockResolvedValueOnce(new Response(null,{status:304}));
 const loader=new WebFrameLoader(request,()=>at),first=await loader.read('QQQ'),next=await loader.read('QQQ');
 expect(next).toBe(first);expect(next.frame).toEqual(frame().frame);expect(request.mock.calls[1][1].headers).toEqual({'If-None-Match':'W/"one"'});
});
it('merges duplicate simultaneous reads into one request',async()=>{
 let resolve!:(r:Response)=>void;const request=vi.fn(()=>new Promise<Response>(r=>resolve=r));const loader=new WebFrameLoader(request,()=>at);
 const a=loader.read('QQQ'),b=loader.read('QQQ');expect(a).toBe(b);resolve(new Response(JSON.stringify(frame())));await a;expect(request).toHaveBeenCalledTimes(1);
});
it('cancels superseded asset work and ignores a late response even when transport ignores abort',async()=>{
 const resolve:((r:Response)=>void)[]=[];const signals:AbortSignal[]=[];
 const request=vi.fn((_:unknown,init?:RequestInit)=>{signals.push(init!.signal!);return new Promise<Response>(r=>resolve.push(r));});
 const loader=new WebFrameLoader(request,()=>at),qqq=loader.read('QQQ');const rejected=expect(qqq).rejects.toMatchObject({name:'AbortError'});const spy=loader.read('SPY');
 expect(signals[0].aborted).toBe(true);resolve[0](new Response(JSON.stringify(frame())));resolve[1](new Response(JSON.stringify(frame('SPY'))));await rejected;await spy;expect(loader.peek('QQQ')).toBeUndefined();expect(loader.peek('SPY')?.frame.symbol).toBe('SPY');
});
it('keeps only two assets and does not use a different asset for missing data',async()=>{
 const request=vi.fn(async(input:unknown)=>new Response(JSON.stringify(frame(String(input).split('=').at(-1)))));const loader=new WebFrameLoader(request,()=>at);
 for(const symbol of ['QQQ','SPY','AAPL'])await loader.read(symbol);
 expect(loader.peek('QQQ')).toBeUndefined();expect(loader.peek('SPY')?.frame.symbol).toBe('SPY');
});
it.each(['foreign','future','bad-publication'])('rejects %s captures before caching',async kind=>{
 const f=frame();if(kind==='foreign')f.frame.symbol='SPY';if(kind==='future')f.capturedMs=at+1;if(kind==='bad-publication')f.frame.publishedMs=at;
 const loader=new WebFrameLoader(vi.fn(async()=>new Response(JSON.stringify(f))),()=>at);await expect(loader.read('QQQ')).rejects.toThrow('Lote inválido');expect(loader.peek('QQQ')).toBeUndefined();
});
it('retains an earlier capture when revalidation fails and propagates quota status',async()=>{
 const request=vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(frame()))).mockResolvedValueOnce(new Response('{"error":"database_quota_exceeded"}',{status:503}));const loader=new WebFrameLoader(request,()=>at),old=await loader.read('QQQ');
 await expect(loader.read('QQQ')).rejects.toMatchObject({code:'database_quota_exceeded'});expect(loader.peek('QQQ')).toBe(old);
});
it('does not treat an unsolicited 304 as a valid frame',async()=>{
 const loader=new WebFrameLoader(vi.fn(async()=>new Response(null,{status:304})),()=>at);await expect(loader.read('QQQ')).rejects.toThrow('Captura no disponible');
});
