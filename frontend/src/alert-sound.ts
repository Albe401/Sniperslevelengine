export const alertSounds={classic:'Clásico',double:'Doble aviso',bell:'Campana',urgent:'Urgente'} as const;
export type AlertSound=keyof typeof alertSounds;
export interface AlertSoundSettings {tone:AlertSound;volume:number}
const key='snipers.alert-sound.v1';
let audio:AudioContext|undefined;
export function validAlertSoundSettings(value:unknown):AlertSoundSettings {
 const v=value as Partial<AlertSoundSettings>|null;
 return {tone:v?.tone&&Object.hasOwn(alertSounds,v.tone)?v.tone:'classic',volume:typeof v?.volume==='number'&&Number.isFinite(v.volume)?Math.max(0,Math.min(100,v.volume)):60};
}
export function readAlertSoundSettings():AlertSoundSettings {
 try{return validAlertSoundSettings(JSON.parse(localStorage.getItem(key)??'null'));}catch{return validAlertSoundSettings(null);}
}
let settings=readAlertSoundSettings();
export function getAlertSoundSettings(){return {...settings};}
export function saveAlertSoundSettings(value:unknown):boolean {
 settings=validAlertSoundSettings(value);
 try{localStorage.setItem(key,JSON.stringify(settings));return true;}catch{return false;}
}
export function alertSoundNotes(tone:AlertSound):{at:number;frequency:number;duration:number;type:OscillatorType}[] {
 switch(tone){
 case 'double':return [0,.3].map(at=>({at,frequency:880,duration:.22,type:'sine'}));
 case 'bell':return [{at:0,frequency:1046.5,duration:.65,type:'sine'},{at:.16,frequency:1568,duration:.55,type:'sine'}];
 case 'urgent':return [0,.2,.4].map((at,i)=>({at,frequency:i%2?1100:740,duration:.15,type:'triangle'}));
 default:return [{at:0,frequency:880,duration:.65,type:'sine'}];
 }
}
export async function enableAlertSound():Promise<boolean>{
 try{audio??=new AudioContext();await audio.resume();return audio.state==='running';}catch{return false;}
}
/** Call resume only from a user gesture; market events never fabricate user activation. */
export function playAlertSound(tone:AlertSound=settings.tone):boolean {
 if(settings.volume===0)return true;
 if(audio?.state!=='running')return false;
 try{
  for(const note of alertSoundNotes(tone)){
   const oscillator=audio.createOscillator(),gain=audio.createGain(),start=audio.currentTime+note.at;
   oscillator.type=note.type;oscillator.frequency.value=note.frequency;oscillator.connect(gain);gain.connect(audio.destination);
   gain.gain.setValueAtTime(.001,start);gain.gain.linearRampToValueAtTime(.2*settings.volume/100,start+.01);
   gain.gain.exponentialRampToValueAtTime(.001,start+note.duration);
   oscillator.onended=()=>{oscillator.disconnect();gain.disconnect();};oscillator.start(start);oscillator.stop(start+note.duration);
  }
  return true;
 }catch{return false;}
}
