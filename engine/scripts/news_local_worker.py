"""Local English -> Spanish news translation. JSON in/out; no network access."""
import html
import json
import os
from pathlib import Path
import re
import sys
import time

MODEL_VERSION = "helsinki-opus-en-es-5bc4493-ct2-int8-v2"


def plain(text):
    return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]*>", " ", str(text)))).strip()


# Taxonomy V2: project heuristics, not a provider classification or probability.
def classify(title, summary):
    def one(value):
        text = plain(value).lower()
        uncertain = bool(re.search(r'\b(not|never|unlikely|could|may|might|if|rumou?r|potential)\b', text))
        pos = bool(re.search(r'\b(beats? (estimates|expectations|consensus)|revenue beat|eps beat|guidance raise|raises? (?:its )?(?:full.year )?(?:eps |revenue )?(guidance|outlook|forecast|range)|raised (guidance|outlook|forecast)|record (revenue|profit)|fda approves|fda approval)\b', text))
        neg = bool(re.search(r'\b(miss(es|ed)? (estimates|expectations|consensus)|(?:cuts?|lowers?|reduced|withdraws?) (?:its )?(?:full.year )?(?:eps |revenue )?(guidance|outlook|forecast|range)|bankruptcy|fraud|fda rejects|fda rejection|trial miss)\b', text))
        reaction_pos = bool(re.search(r'\bshares (rise|rose|surge|surged|jump|jumped)\b', text))
        reaction_neg = bool(re.search(r'\bshares (fall|fell|drop|dropped|plunge|plunged)\b', text))
        direction = 'unknown' if uncertain else 'mixed' if pos and neg else 'positive' if pos else 'negative' if neg else 'unknown'
        reaction = 'mixed' if reaction_pos and reaction_neg else 'positive' if reaction_pos else 'negative' if reaction_neg else 'unknown'
        return text, uncertain, direction, reaction
    ht, hu, hd, hr = one(title)
    st, su, sd, sr = one(summary)
    known = {d for d in (hd, sd) if d != 'unknown'}
    conflict = 'mixed' in known or ('positive' in known and 'negative' in known)
    direction = 'mixed' if conflict else next(iter(known), 'unknown')
    text = ht + ' ' + st
    event = 'other'
    if not hu:
        up = bool(re.search(r'\b(upgrades|upgraded)\b', ht))
        down = bool(re.search(r'\b(downgrades|downgraded)\b', ht))
        analyst = bool(re.search(r'\b(analysts?|rating|rated|overweight|underweight|outperform|underperform|buy|sell|neutral|equal.weight)\b', ht))
        if analyst and up != down and not re.search(r'\btop \d+ (upgrades|downgrades)\b', ht):
            event = 'upgrade' if up else 'downgrade'
    if event != 'other':
        hd = 'positive' if event == 'upgrade' else 'negative'
        known = {d for d in (hd, sd) if d != 'unknown'}
        conflict = 'mixed' in known or ('positive' in known and 'negative' in known)
        direction = 'mixed' if conflict else next(iter(known), 'unknown')
    taxonomy = [
        ('TRADING_HALT', r'\b(trading halt|halted|resumed|resumption)\b'),
        ('BANKRUPTCY', r'\b(bankruptcy|chapter 11)\b'),
        ('FDA', r'\b(fda|clinical trial)\b'),
        ('GUIDANCE', r'\b(guidance|outlook|forecast|full.year eps)\b'),
        ('EARNINGS', r'\b(earnings|quarterly results|revenue beat|eps beat|record revenue)\b'),
        ('ANALYST_ACTION', r'\b(price target|analyst|rating)\b'),
        ('M&A', r'\b(acquisition|acquires|merger|takeover)\b'),
        ('LEGAL', r'\b(lawsuit|litigation|fraud|doj|subpoena)\b'),
        ('REGULATORY', r'\b(sec investigation|regulatory|antitrust)\b'),
        ('MANAGEMENT', r'\b(ceo|cfo|resigns|resignation)\b'),
        ('FINANCING', r'\b(secondary offering|convertible notes|debt offering)\b'),
        ('DIVIDEND', r'\bdividend\b'), ('BUYBACK', r'\b(buyback|share repurchase)\b'),
        ('CONTRACT', r'\b(contract|agreement)\b'), ('PRODUCT', r'\b(product|launch|recall)\b'),
        ('MACRO', r'\b(federal reserve|inflation|interest rates)\b')]
    categories = [c for c, pattern in taxonomy if re.search(pattern, text)]
    category = categories[0] if categories else 'OTHER'
    if event != 'other' and category == 'OTHER': category = 'ANALYST_ACTION'
    subtype = event.upper() if event != 'other' else 'RESUMPTION' if category == 'TRADING_HALT' and re.search(r'\b(resumed|resumption)\b', text) else 'HALT' if category == 'TRADING_HALT' else 'WITHDRAW' if category == 'GUIDANCE' and 'withdraw' in text else 'REPORTED'
    uncertain = hu or su
    critical = not uncertain and (category in ('BANKRUPTCY', 'TRADING_HALT') or category == 'FDA' and bool(re.search(r'\b(approval|approves|rejection|rejects)\b', text)) or category == 'GUIDANCE' and direction == 'negative')
    noise = bool(re.search(r"\b(stocks to watch|best stocks|top \d+|should you|stocks you need|what's going on|how much .{0,80}(invested|owning)|why .{0,50} (stock|shares) (is|are))\b", ht))
    importance = 'high' if (category != 'OTHER' or event != 'other') and not uncertain else 'low' if noise else 'medium'
    # Legacy display can retain observed price reaction; ranking direction uses contentDirection.
    reaction = hr if hr != 'unknown' else sr
    sentiment = direction if direction != 'unknown' else reaction
    return {'sentiment': sentiment, 'contentDirection': direction, 'marketReaction': reaction,
            'importance': importance, 'eventType': event, 'category': category, 'categories': categories,
            'subtype': subtype, 'critical': critical, 'headlineDirection': hd, 'summaryDirection': sd,
            'conflictState': 'MIXED' if conflict else 'NEEDS_REVIEW' if uncertain else 'NONE',
            'classificationVersion': 'news-rules-v2', 'materialityBasis': 'event_type_heuristic; company_relative_magnitude_unknown',
            'reason': 'Lectura conjunta de titular y resumen; conflicto explícito.' if conflict else 'Clasificación local por reglas de titular y resumen; evento reportado, sin verificación primaria.'}


def chunks(text, tokenizer):
    # Sentence boundaries do not split decimal numbers; cap tokens to avoid truncation.
    result = []
    for sentence in re.split(r"(?<=[.!?])\s+(?=[A-Z¿¡])", plain(text)):
        tokens = tokenizer.encode(sentence, out_type=str)
        result.extend(tokens[i:i + 160] for i in range(0, len(tokens), 160))
    return result


def financial_terms(source, value):
    if re.search(r"\bguidance\b", source, re.I):
        value = re.sub(r"\borientación\b", "previsión", value, flags=re.I)
    value = re.sub(r"\bobjetivo de precios\b", "precio objetivo", value, flags=re.I)
    if classify(source, "")["eventType"] != "other" or re.search(r"\b(rating|rated)\b", source, re.I):
        value = re.sub(r"\bactualiza\b", "mejora la calificación de", value, flags=re.I)
        for original, spanish in [("Equal-Weight", "Neutral (igual ponderación)"), ("Overweight", "Sobreponderar"), ("Underweight", "Infraponderar"), ("Outperform", "Mejor que el mercado"), ("Underperform", "Peor que el mercado")]:
            value = re.sub(r"\b" + re.escape(original) + r"\b", spanish, value, flags=re.I)
    return value


def financial_template(text):
    """Exact publisher templates: preserve company names, amounts and clock times."""
    text = plain(text).replace("’", "'")
    ratings = {"outperform": "Mejor que el mercado", "sector outperform": "Mejor que el sector", "market outperform": "Mejor que el mercado", "underperform": "Peor que el mercado", "sector underperform": "Peor que el sector", "buy": "Comprar", "strong buy": "Compra fuerte", "sell": "Vender", "hold": "Mantener", "neutral": "Neutral", "equal-weight": "Neutral (igual ponderación)", "overweight": "Sobreponderar", "underweight": "Infraponderar", "sector perform": "En línea con el sector", "market perform": "En línea con el mercado"}
    match = re.fullmatch(r"(.+?) (Upgrades|Downgrades) (.+?) to (.+?), (.+)", text, re.I)
    maintained = re.fullmatch(r"(.+?) (Maintains|Reiterates) (.+?) on (.+?), (.+)", text, re.I)
    if match or maintained:
        firm, action, company, rating, target = match.groups() if match else (maintained[1], maintained[2], maintained[4], maintained[3], maintained[5])
        rating_es = ratings.get(rating.lower())
        price = re.fullmatch(r"(Raises|Lowers|Maintains|Cuts) Price Target (?:to|at) ([A-Z]*\$[\d,.]+)", target, re.I)
        announced = re.fullmatch(r"Announces ([A-Z]*\$[\d,.]+) Price Target", target, re.I)
        if rating_es and (price or announced):
            verb = {"upgrades": "mejora", "downgrades": "rebaja", "maintains": "mantiene", "reiterates": "reitera"}[action.lower()]
            statement = f"{firm} {verb} la calificación de {company} a {rating_es}"
            if price:
                verb = {"raises": "eleva", "lowers": "reduce", "cuts": "recorta", "maintains": "mantiene"}[price[1].lower()]
                return statement + f"; {verb} el precio objetivo a {price[2]}"
            return statement + f"; anuncia un precio objetivo de {announced[1]}"
    match = re.fullmatch(r"Trading Halt: Halted at (.*?) - Trading Halt: Halt News Pending", text, re.I)
    if match:
        return "Suspensión de cotización a las " + match[1] + " · Pendiente de noticias"
    match = re.fullmatch(r"Trading Halt: Halt status updated at (.*?): Quotation Resumption: IPO security released for quotation", text, re.I)
    if match:
        return "Estado actualizado a las " + match[1] + ": se reanudan las cotizaciones del valor de la salida a bolsa"
    match = re.fullmatch(r"Here(?:'s| Is) How Much (.+?) Invested In (.+?) (\d+) Years Ago Would Be Worth Today", text, re.I)
    if match:
        return f"Cuánto valdrían hoy {match[1]} invertidos en {match[2]} hace {match[3]} años"
    match = re.fullmatch(r"Here(?:'s| Is) How Much You (?:Would Have|Would've) Made Owning (.+?) Stock In The Last (\d+) Years", text, re.I)
    if match:
        return f"Cuánto habrías ganado con acciones de {match[1]} durante los últimos {match[2]} años"
    return None


def translate(items, model_dir):
    import ctranslate2
    import sentencepiece
    tokenizer = sentencepiece.SentencePieceProcessor(model_file=str(model_dir / "source.spm"))
    target_tokenizer = sentencepiece.SentencePieceProcessor(model_file=str(model_dir / "target.spm"))
    translator = ctranslate2.Translator(str(model_dir / "model"), device="cpu", compute_type="int8", intra_threads=min(4, os.cpu_count() or 1), inter_threads=2)
    unique = {}
    sequences = []
    for item in items:
        for field in ("title", "summary"):
            text = plain(item.get(field, ""))
            if text not in unique:
                if financial_template(text) is not None or text == "Solo titular; consulta la fuente original.":
                    unique[text] = (0, 0)
                    continue
                segments = chunks(text, tokenizer)
                unique[text] = (len(sequences), len(segments))
                sequences.extend(segment + ["</s>"] for segment in segments)
    decoded = []
    for offset in range(0, len(sequences), 32):
        output = translator.translate_batch(sequences[offset:offset + 32], beam_size=2, max_decoding_length=320, repetition_penalty=1.05)
        decoded.extend(target_tokenizer.decode(row.hypotheses[0]) for row in output)
    translated = {text: financial_template(text) or (text if text == "Solo titular; consulta la fuente original." else " ".join(decoded[start:start + count])) for text, (start, count) in unique.items()}
    rows = []
    for item in items:
        row = {"id": item["id"], **classify(item.get("title", ""), item.get("summary", ""))}
        for field in ("title", "summary"):
            source = plain(item.get(field, ""))
            value = financial_terms(source, translated[source])
            if source and not value:
                raise ValueError("Traducción local vacía")
            row[field] = value
        rows.append(row)
    return rows


def main():
    started = time.monotonic()
    payload = json.load(sys.stdin)
    items = payload["items"]
    if not isinstance(items, list) or len(items) > 600:
        raise ValueError("Lote local fuera de límite")
    rows = translate(items, Path(sys.argv[1]))
    json.dump({"items": rows, "model": MODEL_VERSION, "seconds": round(time.monotonic() - started, 3)}, sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    sys.stdin.reconfigure(encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        main()
    except Exception as error:
        print("Traductor local: " + str(error), file=sys.stderr)
        sys.exit(1)


