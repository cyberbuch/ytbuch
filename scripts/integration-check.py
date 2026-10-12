#!/usr/bin/env python3
"""Интеграционна проверка на счетоводството и склада срещу работещото API.

Създава демо данни (префикс „Проверка“) и ги оставя в базата — няма изтриване.
Тече срещу dev стека: python3 scripts/integration-check.py
Среда: YTBUCH_API, YTBUCH_CHECK_USER, YTBUCH_CHECK_PASSWORD.
"""

import json
import os
import sys
import urllib.error
import urllib.request

BASE = os.environ.get("YTBUCH_API", "http://127.0.0.1:8092")
USER = os.environ.get("YTBUCH_CHECK_USER", "superadmin")
PASSWORD = os.environ.get("YTBUCH_CHECK_PASSWORD", "123+123")

TOKEN = ""
PASSED = 0
FAILED = 0
CREATED = []


def check(label, cond, detail=""):
    global PASSED, FAILED
    if cond:
        PASSED += 1
        print(f"  ok   {label}")
    else:
        FAILED += 1
        print(f"  FAIL {label} {detail}")


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    headers = {}
    if TOKEN:
        headers["Authorization"] = f"Bearer {TOKEN}"
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            raw = r.read()
            try:
                return r.status, json.loads(raw)
            except Exception:  # noqa: BLE001 — двоичен отговор
                return r.status, raw
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:  # noqa: BLE001
            return e.code, raw
    except Exception as e:  # noqa: BLE001
        return 0, {"detail": str(e)}


def need(label, status, want, body):
    detail = body.get("detail", body) if isinstance(body, dict) else body
    check(label, status == want, f"(очакван {want}, получен {status}: {detail})")
    return status == want


def money(x):
    return f"{x:.2f}"


def main():
    global TOKEN
    print(f"Интеграционна проверка срещу {BASE}")

    # --- вход ---
    status, body = api("POST", "/v1/auth/token", {"sub": USER, "password": PASSWORD})
    TOKEN = (body or {}).get("access_token", "") if isinstance(body, dict) else ""
    check("вход", status == 200 and TOKEN, f"({status})")
    if not TOKEN:
        print("Без вход няма проверка.")
        return 1

    # --- сметки ---
    status, body = api("GET", "/v1/accounts?limit=1000")
    by_num = {a.get("number"): a.get("id") for a in (body or {}).get("items", [])}
    acc = {n: by_num.get(n) for n in ("302", "401", "411", "501", "601", "702")}
    check("сметкопланът има 302/401/411/501/601/702", all(acc.values()), str(acc))

    # --- контрагенти ---
    status, cust = api("POST", "/v1/counterparts", {
        "name": "Проверка Клиент ЕООД", "counterpart_type": "customer",
        "eik": "111111119", "vat_number": "BG111111119", "country": "BG",
    })
    if need("създаване на клиент", status, 201, cust):
        CREATED.append(f"клиент #{cust['id']}")
    status, supp = api("POST", "/v1/counterparts", {
        "name": "Проверка Доставчик ООД", "counterpart_type": "supplier",
        "eik": "222222229", "vat_number": "BG222222229", "country": "BG",
    })
    if need("създаване на доставчик", status, 201, supp):
        CREATED.append(f"доставчик #{supp['id']}")

    # --- стоки ---
    status, goods = api("POST", "/v1/products", {
        "name": "Проверка стока", "code": "ПРОВ-СТ", "unit": "C62",
        "price": "50.00", "cost_price": "30.00", "vat_rate": "20",
        "is_service": 0, "is_inventory": 1,
        "inventory_account_id": acc["302"], "revenue_account_id": acc["702"],
    })
    if need("създаване на стока", status, 201, goods):
        CREATED.append(f"стока #{goods['id']}")
    status, serv = api("POST", "/v1/products", {
        "name": "Проверка услуга", "code": "ПРОВ-УСЛ", "unit": "HUR",
        "price": "100.00", "vat_rate": "20", "is_service": 1, "is_inventory": 0,
        "revenue_account_id": acc["702"],
    })
    if need("създаване на услуга", status, 201, serv):
        CREATED.append(f"услуга #{serv['id']}")

    # --- счетоводство: покупка ---
    buy_lines = [{
        "product_id": goods.get("id"), "description": "Проверка стока",
        "quantity": "10", "unit": "C62", "unit_price": "30.00", "vat_rate": "20",
        "net_amount": "300.00", "vat_amount": "60.00", "total_amount": "360.00",
    }]
    status, buy = api("POST", "/v1/invoices", {
        "direction": "in", "document_type": "01", "number": "ПРОВ-0001",
        "issue_date": "2026-10-05", "counterpart_id": supp.get("id"),
        "location_id": 1, "currency": "EUR",
        "net_amount": "300.00", "vat_amount": "60.00",
        "total_amount": "360.00", "lines": buy_lines,
    })
    if need("покупка — чернова", status, 201, buy):
        CREATED.append(f"покупка #{buy['id']} ПРОВ-0001")
        status, post = api("POST", f"/v1/invoices/{buy['id']}/post")
        if need("покупка — пост", status, 201, post):
            # Отговорът на поста е самият запис в дневника с редовете.
            check("покупката има запис в дневника",
                  isinstance(post, dict) and post.get("id") and post.get("lines"),
                  str(post)[:120])

    # --- счетоводство: продажба ---
    sale_lines = [
        {"product_id": goods.get("id"), "description": "Проверка стока",
         "quantity": "5", "unit": "C62", "unit_price": "50.00", "vat_rate": "20",
         "net_amount": "250.00", "vat_amount": "50.00", "total_amount": "300.00"},
        {"product_id": serv.get("id"), "description": "Проверка услуга",
         "quantity": "2", "unit": "HUR", "unit_price": "100.00", "vat_rate": "20",
         "net_amount": "200.00", "vat_amount": "40.00", "total_amount": "240.00"},
    ]
    status, sale = api("POST", "/v1/invoices", {
        "direction": "out", "document_type": "01",
        "issue_date": "2026-10-06", "counterpart_id": cust.get("id"),
        "location_id": 1, "currency": "EUR",
        "net_amount": "450.00", "vat_amount": "90.00",
        "total_amount": "540.00", "lines": sale_lines,
    })
    if need("продажба — чернова", status, 201, sale):
        CREATED.append(f"продажба #{sale['id']} {sale.get('number', '')}")
        status, post = api("POST", f"/v1/invoices/{sale['id']}/post")
        need("продажба — пост", status, 201, post)

    # --- дневник: записите са там и редовете се балансират ---
    status, journal = api("GET", "/v1/journal?limit=500")
    items = (journal or {}).get("items", [])
    nums = " ".join(str(i.get("document_number", "")) for i in items)
    check("дневникът съдържа ПРОВ-0001", "ПРОВ-0001" in nums)
    sale_no = str(sale.get("number", "")) if isinstance(sale, dict) else ""
    check("дневникът съдържа продажбата", sale_no and sale_no in nums, sale_no)
    entry = next((i for i in items if i.get("document_number") == "ПРОВ-0001"), None)
    if entry:
        status, detail = api("GET", f"/v1/journal/{entry.get('id')}")
        lines = (detail or {}).get("lines", [])
        debit = sum(float(l.get("amount", 0)) for l in lines if l.get("direction") == "debit")
        credit = sum(float(l.get("amount", 0)) for l in lines if l.get("direction") == "credit")
        check("записът на покупката е балансиран", debit > 0 and abs(debit - credit) < 0.01,
              f"дебит {debit} срещу кредит {credit}")

    # --- ДДС регистър ---
    status, vat = api("GET", "/v1/vat/registers?period=2026-10")
    need("ДДС регистър за 2026-10", status, 200, vat)

    # --- справки ---
    for kind in ("trial_balance", "general_ledger", "chronological"):
        status, rep = api("GET", f"/v1/reports?kind={kind}&from=2026-10-01&to=2026-10-31")
        need(f"справка {kind}", status, 200, rep)

    # --- складови правила ---
    status, rule = api("POST", "/v1/stock/rules", {
        "kind": "goods", "inventory_account_id": acc["302"],
        "offset_account_id": acc["702"], "clearing_account_id": 0,
    })
    need("складово правило за стоки", status, 201, rule)
    status, rule = api("POST", "/v1/stock/rules", {
        "kind": "material", "inventory_account_id": acc["302"],
        "offset_account_id": acc["601"], "clearing_account_id": 0,
    })
    need("складово правило за материали", status, 201, rule)

    # --- вътрешен протокол между обектите ---
    status, doc = api("POST", "/v1/internal-docs", {
        "doc_date": "2026-10-06", "from_location_id": 1, "to_location_id": 2,
        "handed_by": "Проверка", "received_by": "Проверка",
        "notes": "Прехвърляне за интеграционна проверка",
        "lines": [{"line_kind": "product", "product_id": goods.get("id"),
                   "quantity": "4", "unit_cost": "30.00", "amount": "120.00",
                   "description": "Проверка стока"}],
    })
    if need("вътрешен протокол", status, 201, doc):
        CREATED.append(f"протокол #{doc['id']}")
        status, conf = api("POST", f"/v1/internal-docs/{doc['id']}/confirm")
        check("протоколът е потвърден", status in (200, 201), f"({status})")

    # Количеството се мести при преизчислението (вж. docs).
    status, reb = api("POST", "/v1/stock/rebuild", {})
    need("преизчисление на склада", status, 200, reb)
    status, inv2 = api("GET", "/v1/inventory/at-location?location_id=2")
    have = {i.get("product_id") for i in (inv2 or {}).get("items", [])}
    check("наличност в обект 2 след преизчислението", goods.get("id") in have, str(have))

    # --- изписване на материали ---
    status, issue = api("POST", "/v1/material-issues", {
        "doc_date": "2026-10-07", "location_id": 2, "notes": "Проверка изписване",
        "lines": [{"product_id": goods.get("id"), "quantity": "2",
                   "account_id": acc["601"], "inventory_account_id": acc["302"]}],
    })
    if need("изписване на материали", status, 201, issue):
        CREATED.append(f"изписване #{issue['id']}")
        status, conf = api("POST", f"/v1/material-issues/{issue['id']}/confirm")
        check("изписването е потвърдено", status in (200, 201), f"({status})")

    # --- складова справка ---
    status, rep = api("GET", "/v1/stock/report?from=2026-10-01&to=2026-10-31")
    need("складова справка", status, 200, rep)

    # --- ръчен запис в дневника ---
    status, manual = api("POST", "/v1/journal", {
        "entry_date": "2026-10-08", "document_number": "ПРОВ-РЪЧЕН",
        "description": "Проверка ръчен запис",
        "lines": [
            {"account_id": acc["501"], "direction": "debit", "amount": "100.00"},
            {"account_id": acc["702"], "direction": "credit", "amount": "100.00"},
        ],
    })
    if need("ръчен запис в дневника", status, 201, manual):
        CREATED.append(f"ръчен запис #{manual.get('id')}")

    # --- печат и експорти ---
    if isinstance(sale, dict) and sale.get("id"):
        status, pdf = api("GET", f"/v1/invoices/{sale['id']}/print?format=pdf")
        detail = pdf.get("detail") if isinstance(pdf, dict) else f"({status})"
        check("PDF на продажбата", status == 200 and isinstance(pdf, bytes) and pdf[:4] == b"%PDF",
              str(detail))
        status, odt = api("GET", f"/v1/invoices/{sale['id']}/print?format=odt")
        detail = odt.get("detail") if isinstance(odt, dict) else f"({status})"
        check("ODT на продажбата", status == 200 and isinstance(odt, bytes) and odt[:2] == b"PK",
              str(detail))
    status, saft = api("GET", "/v1/saft/export?period=2026-10-01&mode=monthly")
    detail = saft.get("detail") if isinstance(saft, dict) else f"({status})"
    check("SAF-T XML", status == 200 and isinstance(saft, bytes) and b"AuditFile" in saft,
          str(detail))
    status, xlsx = api("GET", "/v1/reports/export?kind=trial_balance&from=2026-10-01"
                      "&to=2026-10-31&format=xlsx&bg=1")
    detail = xlsx.get("detail") if isinstance(xlsx, dict) else f"({status})"
    check("XLSX на оборотната ведомост", status == 200 and isinstance(xlsx, bytes)
          and xlsx[:2] == b"PK", str(detail))

    print(f"\nРезултат: {PASSED} успешно, {FAILED} с грешка")
    print("Създадени демо данни (остават в базата):")
    for c in CREATED:
        print(f"  - {c}")
    return 1 if FAILED else 0


if __name__ == "__main__":
    sys.exit(main())
