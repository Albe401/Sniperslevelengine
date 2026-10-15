import type {BotSession} from './paper-bot';
import type {ChartMark} from './chart';
import type {DecisionReview} from './decision-review';
export function paperChartMarks(review:DecisionReview|undefined,symbol:string):ChartMark[] {
  const marks:ChartMark[]=[];
  function add(trade:{entry:number;current:number;openedMs?:number;closedMs?:number|null},label:string){
    if(trade.openedMs!=null)marks.push({timestampMs:trade.openedMs,price:trade.entry,label:`Entrada simulada ${label}`,kind:'entry'});
    if(trade.closedMs!=null)marks.push({timestampMs:trade.closedMs,price:trade.current,label:`Salida simulada ${label}`,kind:'exit'});
  }
  for(const p of review?.positionHistory??[])if(p.symbol===symbol)add(p.trade,`Rust #${p.id}`);
  if(!review?.positionHistory&&review?.positionSymbol===symbol&&review.position?.trade)add(review.position.trade,`Rust #${review.position.tradeId}`);
  for(const p of review?.legacyPositions??[])if(p.symbol===symbol)add(p.trade,`Python #${p.legacyId}`);
  return marks;
}

/** Broker fill times, never submission times or option premiums on the underlying axis. */
export function brokerChartMarks(history:BotSession[],current:BotSession|undefined,symbol:string,now=Date.now()):ChartMark[] {
 const sessions=new Map(history.map(s=>[s.buy_id,s]));
 if(current)sessions.set(current.buy_id,current);
 const marks:ChartMark[]=[];
 for(const s of sessions.values()){
  const underlying=s.symbol.replace(/^O:/,'').match(/^([A-Z]+)\d{6}[CP]\d{8}$/)?.[1];
  if(underlying!==symbol)continue;
  const entered=Date.parse(s.entry_time??'');
  if(!Number.isFinite(entered)||entered<=0||entered>now||!Number.isFinite(s.entry_premium)||s.entry_premium!<=0)continue;
  const detail=`Alpaca Paper · ${s.side??''} · ${s.symbol}`;
  marks.push({timestampMs:entered,kind:'entry',label:`Entrada ${detail} · prima $${s.entry_premium!.toFixed(2)}`,source:'broker'});
  const exited=Date.parse(s.exit_time??'');
  if(Number.isFinite(exited)&&exited>=entered&&exited<=now&&Number.isFinite(s.exit_premium)&&s.exit_premium!>0)
   marks.push({timestampMs:exited,kind:'exit',label:`Salida ${detail} · prima $${s.exit_premium!.toFixed(2)} · ${s.exit_trigger??s.reason}`,source:'broker'});
 }
 return marks;
}
