import {it,expect,vi} from 'vitest';
vi.mock('./live',()=>({liveSnapshotForOvernight:()=>({state:undefined,active:false})}));
import {overnightPage} from './overnight-page';
it('shows six separate readings and keeps an uncalibrated result pending without saved data',()=>{const html=overnightPage();for(const name of ['Bloques institucionales','Cierre y reacción','Confirmación premarket','Componentes QQQ','Lectura ensamblada','Validación y aprendizaje'])expect(html).toContain(name);expect(html).toContain('ÍNDICE DE EVIDENCIA');expect(html).toContain('PENDIENTE');expect(html).not.toContain('probabilidad de llenar');});
