/** OCC series identity; adjusted roots remain distinct but retain the same encoded expiry/side/strike. */
export function optionSeriesMatches(c:{ticker:string;expiration:string;kind:string;strike:number},symbol?:string):boolean {
 if(typeof c.ticker!=='string'||!c.ticker.length)return false;
 // Synthetic/internal identifiers have no OCC encoding and remain snapshot-only evidence.
 if(!c.ticker.startsWith('O:'))return true;
 const m=/^O:(.+)(\d{6})([CP])(\d{8})$/.exec(c.ticker);
 if(!m)return false;
 const date=`20${m[2].slice(0,2)}-${m[2].slice(2,4)}-${m[2].slice(4,6)}`;
 const ms=Date.parse(date+'T12:00:00Z');
 if(!Number.isFinite(ms)||new Date(ms).toISOString().slice(0,10)!==date||c.expiration!==date||c.kind!==(m[3]==='C'?'call':'put')||c.strike!==Number(m[4])/1000)return false;
 if(symbol){const base=symbol==='BRK.B'?'BRKB':symbol;const root=m[1];if(root!==base&&!(symbol==='SPX'&&root==='SPXW')&&!(root.startsWith(base)&&/^\d+$/.test(root.slice(base.length))))return false;}
 return true;
}
