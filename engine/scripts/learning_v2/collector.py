"""Passive collector prepared for integration; caller supplies shared feed frames."""
from .adapter import from_live


class Collector:
    def __init__(self, store):
        self.store=store

    def ingest(self, state, decision, received_ms):
        """Advance every received symbol, regardless of which view is selected.

        Follow-up subscription demand is exposed via required_symbols(); no new
        network connection or subscriptions are started by this module.
        """
        reading=from_live(state,decision,received_ms)
        tick=state.get('ticks',{}).get(reading.symbol) or {}
        self.store.expire(received_ms)
        if state.get('stocks',{}).get('phase')=='streaming' and tick.get('symbol')==reading.symbol:
            self.store.advance(reading.symbol,reading.session,reading.phase,tick.get('price'),tick.get('tradeMs'),received_ms)
        previous=self.store.db.execute('SELECT MAX(at) FROM v2_readings WHERE symbol=? AND session=? AND phase=?',(reading.symbol,reading.session,reading.phase)).fetchone()[0]
        if previous is not None and received_ms//30000<=previous//30000:
            return []
        ids=[]
        # All horizons are recorded atomically; capacity errors cannot leave half a cycle.
        readings=[from_live(state,decision,received_ms,h) for h in (300000,900000,1800000)]
        with self.store.db:
            for r in readings:
                if self.store.record(r,_commit=False):ids.append(r.id)
        return ids

    def required_symbols(self):
        return self.store.required_symbols()
