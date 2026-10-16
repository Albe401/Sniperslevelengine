const easternWeekday=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',weekday:'short'});
export function webAutoRefreshAllowed(now=Date.now()):boolean {
 if(!Number.isFinite(now))return false;
 const day=easternWeekday.format(new Date(now));
 return day!=='Sat'&&day!=='Sun';
}
export function webRefreshLabel(now=Date.now()):string {
 return webAutoRefreshAllowed(now)?'actualización cada 5 min · lunes a viernes ET':'actualización automática pausada · fin de semana ET';
}
