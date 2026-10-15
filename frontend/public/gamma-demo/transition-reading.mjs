// Demo-only, causal reading: no future samples, orders or dealer-inventory claims.
export function transitionReading({path,flow,vwap}) {
 const wait={kind:'waiting',headline:'ESPERANDO UN CAMBIO CLARO',detail:'Falta continuidad del precio para definir el movimiento.',bias:0};
 if(!Array.isArray(path)||path.length<5||!path.every(p=>Number.isFinite(p)&&p>0)||!Number.isFinite(flow)||!Number.isFinite(vwap)||vwap<=0)return wait;
 const p=path.slice(-5),returns=p.slice(1).map((v,i)=>v-p[i]);
 const noise=Math.max(p.at(-1)*.00012,.04);
 const previous=returns[0]+returns[1],recent=returns[2]+returns[3];
 const range=Math.max(...p.slice(-4))-Math.min(...p.slice(-4));
 const up=returns[2]>noise&&returns[3]>noise,down=returns[2]<-noise&&returns[3]<-noise;
 const above=p.slice(-2).every(v=>v>vwap+noise),below=p.slice(-2).every(v=>v<vwap-noise);
 if(range<=noise*3)return {kind:'compression',headline:'PRECIO COMPRIMIDO · DIRECCIÓN PENDIENTE',detail:'Hay poco desplazamiento. Esperando una salida sostenida del rango.',bias:0};
 if(up&&flow>.2&&above)return {kind:'call',headline:'CAMBIO HACIA CALL · CONFIRMADO EN EL DEMO',detail:'El precio sostiene la subida y recupera el VWAP, acompañado por flujo comprador simulado.',bias:32};
 if(down&&flow<-.2&&below)return {kind:'put',headline:'CAMBIO HACIA PUT · CONFIRMADO EN EL DEMO',detail:'El precio sostiene la caída bajo el VWAP, acompañado por flujo vendedor simulado.',bias:-32};
 if(previous>noise*2&&recent<previous*.4)return {kind:'exhaust-up',headline:'LA SUBIDA ESTÁ PERDIENDO FUERZA',detail:'El avance se frena o retrocede. Todavía falta confirmar un cambio hacia PUT.',bias:-10};
 if(previous<-noise*2&&recent>previous*.4)return {kind:'exhaust-down',headline:'LA CAÍDA ESTÁ PERDIENDO FUERZA',detail:'La presión bajista deja de producir avance. Todavía falta confirmar un cambio hacia CALL.',bias:10};
 if(up)return {kind:'call-pending',headline:'CAMBIANDO HACIA CALL · EN OBSERVACIÓN',detail:above?'El precio avanza; falta acompañamiento comprador del flujo.':'El precio mejora; falta sostener la recuperación del VWAP y el flujo.',bias:18};
 if(down)return {kind:'put-pending',headline:'CAMBIANDO HACIA PUT · EN OBSERVACIÓN',detail:below?'El precio retrocede; falta acompañamiento vendedor del flujo.':'El precio pierde fuerza; falta sostener la pérdida del VWAP y el flujo.',bias:-18};
 return {...wait,detail:'Los intentos se alternan sin una dirección sostenida. Esperando confirmación.'};
}
