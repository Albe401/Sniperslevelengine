export const snapshotSymbols=['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM'] as const;
export interface SavedBundle {available:boolean;id?:string;capturedMs?:number;frames?:Record<string,Record<string,unknown>>;images?:Record<string,{base64:string;mimeType:string;sha256:string;sourceSha256:string}>;coverage?:Record<string,unknown>;extras?:Record<string,{command:string;args:Record<string,unknown>;observedMs:number;value:unknown}>}
export function validSavedBundle(value:SavedBundle,now:number):boolean {
 if(!value.available||!value.id||!Number.isFinite(value.capturedMs)||value.capturedMs!>now-300000||!value.frames)return false;
 return snapshotSymbols.every(symbol=>{
  const frame=value.frames![symbol];
  return !!frame&&frame.symbol===symbol&&(frame.publishedMs==null||(typeof frame.publishedMs==='number'&&frame.publishedMs<=value.capturedMs!));
 });
}
export function savedAsset(value:SavedBundle|undefined,symbol:string,now:number):Record<string,unknown>|undefined {
 return value&&validSavedBundle(value,now)?value.frames?.[symbol]:undefined;
}
export function snapshotCommandAllowed(command:string):boolean {
 // Native frames already archive live engine data. Do not copy the high-frequency
 // live stream through the UI again; only collect supplemental sections here.
 return ['get_contract_inventory','get_fmp_targets','preview_route_4h'].includes(command);
}
