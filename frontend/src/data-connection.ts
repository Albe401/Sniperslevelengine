export type DataConnectionMode='auto'|'receiver'|'direct';
export function receiverHealth(independent:boolean,publishedMs:number|undefined,disconnected:boolean,now:number):string{
 if(!independent)return '';
 if(disconnected)return 'Receptor sin conexión';
 if(publishedMs==null||!Number.isFinite(publishedMs)||publishedMs>now||now-publishedMs>5000)return 'Lectura del receptor atrasada';
 return 'Receptor activo';
}
/** Abort the transfer on any detach failure: never start two owners. */
export async function transferConnection(detach:()=>Promise<unknown>,attach:()=>Promise<unknown>):Promise<void>{await detach();await attach();}
