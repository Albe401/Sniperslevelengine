import type {DirectionScore} from './direction-scanner';
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const pending=(reason:string):DirectionScore=>({call:null,put:null,side:'PENDIENTE',state:'Esperando datos',reason,components:[]});
export function combineDirection(price:number,flow:number,structure:number,backing:number|null,provisional=false):DirectionScore {
 if([price,flow,structure].some(n=>!finite(n)||Math.abs(n)>1)||backing!=null&&(!finite(backing)||backing<0||backing>100))return pending('Componentes inválidos');
 const components=[{name:'Precio y VWAP',signal:price,weight:40},{name:'Prima clasificada 5m',signal:flow,weight:40},{name:'Walls, imán y barreras',signal:structure,weight:20}];
 let net=components.reduce((v,c)=>v+c.signal*c.weight,0)/100;
 // Weak/unknown backing attenuates a direction; it never flips it by itself.
 net*=backing==null?.5:.5+backing/200;
 if(provisional)net*=.75;
 const call=Math.round(50+50*net),put=100-call;
 const side=call>=60?'CALL':put>=60?'PUT':'NEUTRAL';
 const aligned=side==='CALL'?price>0&&flow>0:side==='PUT'?price<0&&flow<0:false;
 const state=side==='NEUTRAL'?'Sin predominio':!aligned?'Lectura mixta':provisional?'Confluencia parcial':Math.max(call,put)>=75?'Confluencia fuerte':'Preparándose';
 return {call,put,side,state,components,reason:'Confluencia experimental 40/40/20. Respaldo débil y estructura provisional reducen intensidad. No confirma activación ni hora futura.'};
}
