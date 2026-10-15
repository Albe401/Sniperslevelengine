export interface Treasury {phase:string;date:string|null;previousDate:string|null;yieldPercent:number|null;changeBps:number|null;checkedMs:number|null;detail:string}
export function treasuryContext(t:Treasury|undefined,now=Date.now()):{label:string;detail:string} {
 const pending={label:'10Y diario · pendiente',detail:'Esperando rendimiento diario de Massive Economy. No modifica CALL/PUT.'};
 if(!t||!t.date||!t.previousDate||!/^\d{4}-\d{2}-\d{2}$/.test(t.date)||t.previousDate>=t.date||t.checkedMs==null||!Number.isFinite(t.checkedMs)||t.checkedMs>now||t.yieldPercent==null||!Number.isFinite(t.yieldPercent)||t.changeBps==null||!Number.isFinite(t.changeBps))return pending;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const age=Date.parse(today)-Date.parse(t.date);
 if(!Number.isFinite(age)||age<0)return pending;
 const limited=t.phase!=='ready'||age>7*86400000||now-t.checkedMs>7200000;
 const change=Math.abs(t.changeBps)<.05?'0.0':`${t.changeBps>0?'+':''}${t.changeBps.toFixed(1)}`;
 return {label:`10Y diario ${t.yieldPercent.toFixed(2)}% · ${change} pb${limited?' · revisar':''}`,detail:`Observación ${t.date}; cambio frente a ${t.previousDate}. ${limited?'Dato antiguo o consulta no vigente. ':''}Contexto diario de tasas, sin velocidad intradía ni voto en CALL/PUT. ${t.detail}`};
}
