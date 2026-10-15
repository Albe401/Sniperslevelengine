/** Archives already computed outputs. Never invokes a scorer or changes a decision. */
export interface BaselinePacket {
 version:'option-formula-baseline-v1';source:'desktop-live';symbol:string;session:string;
 observedMs:number;publishedMs:number;connection:number;
 confluence:{version:string;call:number|null;put:number|null;side:string;components:{name:string;signal:number;weight:number}[]};
 quant:{version:string;horizonMinutes:10;state:string;at:number;probabilities:number[]|null;sampleId:string;side:number;upper:number|null;lower:number|null};
}
const key='snipers.option-formula-baseline.v1';
export class OptionBaselineArchive {
 private rows:BaselinePacket[]=[];private second=-1;private persistedMinute=-1;
 dropped=0;storageError='';
 constructor(private storage?:Pick<Storage,'getItem'|'setItem'>){
  try{const saved=JSON.parse(storage?.getItem(key)??'null');
   if(saved?.version==='option-formula-baseline-archive-v1'&&Array.isArray(saved.rows)){
    this.rows=saved.rows.filter((r:BaselinePacket)=>r.version==='option-formula-baseline-v1'&&Number.isSafeInteger(r.observedMs)).slice(-10000);
    this.dropped=Number.isSafeInteger(saved.dropped)?saved.dropped:0;
   }
  }catch{this.storageError='Archivo de referencia local no disponible';}
 }
 observe(packet:BaselinePacket){
  if(!Number.isSafeInteger(packet.observedMs)||!Number.isSafeInteger(packet.publishedMs)||packet.publishedMs<=0||packet.publishedMs>packet.observedMs||packet.observedMs-packet.publishedMs>3000)return;
  const second=Math.floor(packet.observedMs/1000);
  if(second<=this.second)return;this.second=second;
  this.rows.push(JSON.parse(JSON.stringify(packet)) as BaselinePacket);
  if(this.rows.length>10000){this.rows.shift();this.dropped++;}
  // Batched persistence keeps research serialization out of every frame.
  const minute=Math.floor(packet.observedMs/60000);
  if(minute!==this.persistedMinute){this.persistedMinute=minute;this.persist();}
 }
 private persist(){
  if(!this.storage)return;
  try{this.storage.setItem(key,JSON.stringify(this.export()));this.storageError='';}
  catch{this.storageError='Referencia en memoria; exportar antes de cerrar la aplicación';}
 }
 export(){return {version:'option-formula-baseline-archive-v1',rows:JSON.parse(JSON.stringify(this.rows)) as BaselinePacket[],dropped:this.dropped,storageError:this.storageError,persistence:'minute-batches; current minute may be memory-only'};}
}
