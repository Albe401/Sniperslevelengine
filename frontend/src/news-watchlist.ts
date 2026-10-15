const defaults=['QQQ','SPY','GOOGL','NVDA','AAPL'];
let symbols=[...defaults];
export const watchedNewsSymbols=()=>[...symbols];
export function setWatchedNewsSymbols(next:string[]):void {
 const clean=[...new Set(next.map(s=>s.trim().toUpperCase()).filter(s=>/^[A-Z0-9.-]{1,12}$/.test(s)))].slice(0,5);
 if(JSON.stringify(clean)===JSON.stringify(symbols))return;
 symbols=clean;
 if(typeof document!=='undefined')document.dispatchEvent(new Event('snipers:news-watchlist'));
}
