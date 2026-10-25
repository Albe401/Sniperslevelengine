import {afterEach,expect,it,vi} from 'vitest';
import real from './structural-map-context.real.json';
import saved from './structural-context-web.saved.json';
import {priceStructureMap} from './price-structure-map';
import * as priceEngine from './price-structure';
import * as context from './structural-map-context';
import type {LiveState} from './live';
afterEach(()=>vi.restoreAllMocks());
it('uses frozen native price outputs through the real snapshot reader and connects their context',()=>{
 const s={...structuredClone(real.state),priceStructureReading:structuredClone(saved)} as unknown as LiveState;
 const raw=vi.spyOn(priceEngine,'priceStructureCards'),joined=vi.spyOn(context,'buildStructuralMapContext');
 const html=priceStructureMap('QQQ',s,true,real.now,[]);expect(raw).not.toHaveBeenCalled();expect(joined).toHaveBeenCalledTimes(1);
 expect(html.match(/data-price-structure-side="PUT"/g)).toHaveLength(4);expect(html.match(/data-price-structure-side="CALL"/g)).toHaveLength(1);expect(html).toContain('Datos conectados y cobertura');expect(html.match(/data-options-context-id=/g)).toHaveLength(6);
});
