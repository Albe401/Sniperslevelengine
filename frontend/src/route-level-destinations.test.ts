import {it,expect} from 'vitest';
import {routeLevelDestinations} from './route-level-destinations';
import type {MarketEvidence} from './market-evidence';
const now=Date.parse('2026-10-09T14:00:00-04:00');
const m=()=>({at:now,price:{ready:true,spot:100},levels:[99,101,102].map(price=>({price,name:'WALL',role:'barrier',observed:now,asof:now,expiration:'2026-10-09',provisional:true,strength:70}))} as MarketEvidence);
it('shows both nearest prospective references before confirmation, preserving provisional strength',()=>{const r=routeLevelDestinations(m(),now);expect(r.map(v=>v.target)).toEqual([101,99]);expect(r.every(v=>v.provisional&&v.strength===70)).toBe(true);expect(r[0].condition).toContain('falta confirmar');expect(r[0]).not.toHaveProperty('probability');});
it('never jumps over a nearer barrier or invents the missing opposite reference',()=>{const f=m();f.levels=f.levels.filter(l=>l.price>100);expect(routeLevelDestinations(f,now).map(v=>v.target)).toEqual([101]);});
it('rejects stale/future prices, old-day references, expired levels and vacuum edges',()=>{for(const at of [now+1,now-3001,NaN])expect(routeLevelDestinations({...m(),at},now)).toEqual([]);for(const change of [{expiration:'2026-10-08'},{asof:now-300001},{observed:now+1},{role:'vacuum-edge'}]){const f=m();f.levels=f.levels.map(l=>({...l,...change})) as MarketEvidence['levels'];expect(routeLevelDestinations(f,now)).toEqual([]);}});
