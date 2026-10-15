export interface StructuralQuality {label:string;detail:string;consistent:boolean;bounds?:[number,number]}
export function structuralQuality(call:number|null,partial:boolean,bounds:unknown,capacity:unknown):StructuralQuality {
 const pending={label:'Datos insuficientes',detail:'No se puede acotar el efecto de los datos faltantes. OI, multiplicador o delta pendientes. Alcance: muestra consultada.',consistent:false};
 if(call==null||!Number.isFinite(call)||call<0||call>100)return pending;
 let interval:[number,number];
 if(!partial)interval=[call,call];
 else {
  if(!Array.isArray(bounds)||bounds.length!==2||bounds.some(v=>typeof v!=='number'||!Number.isFinite(v))||bounds[0]<0||bounds[1]>100||bounds[0]>call+1e-6||bounds[1]<call-1e-6||bounds[0]>bounds[1])return pending;
  interval=[bounds[0],bounds[1]];
 }
 const [low,high]=interval;
 const side=low>=52.5?'CALL':high<=47.5?'PUT':low>47.5&&high<52.5?'Equilibrado':null;
 const cap=typeof capacity==='number'&&Number.isFinite(capacity)&&capacity>=0&&capacity<=100?` · capacidad OI×multiplicador con delta: ${capacity.toFixed(1)}%`:'';
 return {label:side?side==='Equilibrado'?'Equilibrio consistente':`Predominio ${side} consistente`:'Lectura incierta',consistent:!!side,bounds:interval,detail:`CALL posible ${low.toFixed(1)}–${high.toFixed(1)}%${cap}. Límites conservadores usando |delta| ≤ 1, no intervalo estadístico ni probabilidad. Solo describe la muestra consultada; no confirma compra, venta ni dirección futura.`};
}
