import './gamma-demo.css';
export function gammaDemoPage():string {
 return `<section id="gamma-demo-workspace"><button id="gamma-demo-expand">Ampliar panel</button><iframe title="Demo de gamma y recorrido" src="/gamma-demo/mapa-gamma.html" sandbox="allow-scripts allow-same-origin" id="gamma-demo-frame"></iframe></section>`;
}
export function bindGammaDemo():void {
 const root=document.querySelector<HTMLElement>('#gamma-demo-workspace');if(!root)return;
 const button=root.querySelector<HTMLButtonElement>('#gamma-demo-expand')!;
 const toggle=()=>{root.classList.toggle('gamma-demo-expanded');button.textContent=root.classList.contains('gamma-demo-expanded')?'Cerrar vista ampliada':'Ampliar panel';};
 button.onclick=toggle;
}
