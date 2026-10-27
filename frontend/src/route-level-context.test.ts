import {it,expect} from 'vitest';
import {mapRouteLevels,assessRouteStructure} from './route-level-context';
import type {LiveState} from './live';
import type {RouteLevel} from './route-engine';
const now=Date.UTC(2026,9,5,15),expiration='2026-10-05';
const s=()=>({symbol:'QQQ',publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:[expiration],structure:{complete:true,callWall:{strike:745,score:85},putWall:{strike:730,score:45},selected:{M1:740},magnet:740,routes:[{direction:'CALL',barrier:743,barrierScore:80,pullTarget:744,score:70,vacuumLow:747,vacuumHigh:749,vacuumScore:90}]}}]}} as unknown as LiveState);
const level=(role:RouteLevel['role'],price:number,strength:number|null):RouteLevel=>({name:role??'level',role,price,strength,observed:now,expiration,provisional:false});
it('takes exactly the map wall score rather than a different dynamics score',()=>{const state=s();state.levelDynamics={symbol:'QQQ',at:now,available:true,levels:[{name:'PUT WALL',strike:730,strength:90,change:-6,provisional:false}]} as LiveState['levelDynamics'];const row=mapRouteLevels(state,'QQQ',true,now).find(l=>l.role==='put-wall')!;expect(row.strength).toBe(45);expect(row.dynamicStrength).toBe(90);expect(row.change).toBe(-6);});
it('includes barriers, pulls, magnets and vacuum edges',()=>{const rows=mapRouteLevels(s(),'QQQ',true,now);expect(rows.map(l=>l.role)).toEqual(expect.arrayContaining(['call-wall','put-wall','magnet','barrier','pull','vacuum-edge']));});
it.each(['mixed-snapshot','duplicate-day','mixed-expirations'])('rejects %s levels rather than choosing a convenient horizon',kind=>{
 const state=s();if(kind==='mixed-snapshot')state.optionSnapshotStatus.horizons[0].observedMs--;
 if(kind==='duplicate-day')state.optionSnapshotStatus.horizons.push({...state.optionSnapshotStatus.horizons[0]});
 if(kind==='mixed-expirations')state.optionSnapshotStatus.horizons[0].expirations.push('2026-10-06');
 expect(mapRouteLevels(state,'QQQ',true,now)).toEqual([]);
});
it.each(['foreign','stale','future','inactive','next-session'])('rejects unsupported structure: %s',kind=>{const state=s();if(kind==='foreign')state.symbol='SPY';if(kind==='stale')state.publishedMs=now-4000;if(kind==='future')state.optionSnapshotStatus.horizons[0].observedMs=now+1;if(kind==='next-session')state.optionSnapshotStatus.horizons[0].mode='NEXT_SESSION';expect(mapRouteLevels(state,'QQQ',kind!=='inactive',now)).toEqual([]);});
it('weak put wall limits backing without electing a down direction',()=>{const r=assessRouteStructure([level('put-wall',730,45),level('call-wall',745,85)],740,1,null,now);expect(r.backing?.strength).toBe(45);expect(r.requiresAdditionalTest).toBe(true);expect(r.ahead[0].price).toBe(745);});
it('weak call wall is limited backing for a downward route',()=>{const r=assessRouteStructure([level('call-wall',745,30),level('put-wall',730,80)],740,-1,null,now);expect(r.backing?.price).toBe(745);expect(r.requiresAdditionalTest).toBe(true);});
it('a weakening strong wall still requires another price test',()=>{const wall={...level('put-wall',730,85),change:-8};expect(assessRouteStructure([wall],740,1,null,now).requiresAdditionalTest).toBe(true);});
it('missing score is not weak or strong and zero is an observed low score',()=>{expect(assessRouteStructure([level('put-wall',730,null)],740,1,null,now).quality).toBe('unverified');expect(assessRouteStructure([level('put-wall',730,0)],740,1,null,now).quality).toBe('limited');});
it('vacuum intensity never supplies wall backing',()=>{const r=assessRouteStructure([level('vacuum-edge',730,95)],740,1,730,now);expect(r.backing).toBeNull();expect(r.anchor).toBeNull();expect(r.quality).toBe('unverified');});
it('a put wall above price is not lower backing',()=>{expect(assessRouteStructure([level('put-wall',745,20)],740,1,null,now).backing).toBeNull();});
it('provisional strong walls do not certify strong backing',()=>{expect(assessRouteStructure([{...level('put-wall',730,90),provisional:true}],740,1,null,now).quality).toBe('unverified');});
it.each([NaN,101,-1])('rejects malformed scores %s',n=>{expect(assessRouteStructure([level('put-wall',730,n)],740,1,null,now).backing?.strength).toBeNull();});

it.each([
 {strength:59.999,change:0,blocked:true},
 {strength:60,change:0,blocked:false},
 {strength:85,change:-5,blocked:true},
 {strength:85,change:-4.999,blocked:false},
])('separates price continuation from the unchanged backing policy: %o',({strength,change,blocked})=>{
 const wall={...level('put-wall',730,strength),change},saved=JSON.stringify(wall);
 const r=assessRouteStructure([wall],740,1,null,now);
 expect(r.confirmationBlocked).toBe(blocked);expect(r.requiresAdditionalTest).toBe(blocked);
 expect(r.backing?.strength).toBe(strength);expect(JSON.stringify(wall)).toBe(saved);
 if(blocked)expect(r.reason).toContain('otra vela no verifica el respaldo');
});
it('partial evidence still blocks a strong score without inventing new strength or votes',()=>{
 const wall={...level('put-wall',730,85),dynamicStrength:99,evidence:{quality:'partial'} as NonNullable<RouteLevel['evidence']>};
 const r=assessRouteStructure([wall],740,1,null,now);
 expect(r.quality).toBe('limited');expect(r.confirmationBlocked).toBe(true);expect(r.requiresAdditionalTest).toBe(true);
 expect(r.backing?.strength).toBe(85);expect(r.levels).toHaveLength(1);
});
it('keeps unavailable and provisional backing distinct from the existing limited-quality veto',()=>{
 for(const wall of [level('put-wall',730,null),{...level('put-wall',730,85),provisional:true}]){
  const r=assessRouteStructure([wall],740,1,null,now);
  expect(r.quality).toBe('unverified');expect(r.confirmationBlocked).toBe(false);expect(r.requiresAdditionalTest).toBe(false);
 }
});
