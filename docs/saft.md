# SAF-T номенклатури и сметкоплан

Каталогът е същият като в bagabuch: НАП Structure Definition V1.0.1.
Файл: `backend/data/saft_noms.json` (UTF-8, около 104 КБ). Редовете са
текст с `|`. API-то го чете през `include_str!` в `backend/src/saft.rs`.
Таблица за номенклатурите няма.

## Видове

`GET /v1/saft/nomenclatures` без `kind` връща тези 11:

| `kind` | Име | Редове | Полета |
|--------|-----|--------|--------|
| `stock_movements` | Движения запаси | 20 | `code`, `name`, `name_en` |
| `asset_movements` | Движения активи | 13 | `code`, `name`, `name_en` |
| `tax_regimes` | Данъчни режими | 3 | `code`, `name`, `name_en` |
| `payment_methods` | Начини на плащане | 9 | `method_code`, `mechanism_code`, `name`, `name_en` |
| `invoice_types` | Видове фактури | 19 | `code`, `name`, `name_en` |
| `tax_types` | Видове данъци | 21 | `code`, `name` |
| `tax_codes` | Данъчни кодове | 169 | `tax_type`, `code`, `description`, `rate`, `note` |
| `product_types` | Видове запаси | 5 | `code`, `name`, `name_en` |
| `units` | Мерни единици | 127 | `code`, `name`, `name_en`, `symbol`, `category`, `source` |
| `regions` | Области | 28 | `code`, `name` |
| `accounts` | Сметки НАП | 360 | `code`, `name`, `section`, `section_name`, `group`, `group_name` |

Първа сметка в каталога: `101` `Основен капитал, изискващ регистрация`,
раздел 1, група 10. Първи вид фактура: `01` Фактура. Първа мярка: `C62`
Единица, символ `бр.`. Първи данъчен код: `000` / `000000` Неприложим код.

`search` филтрира по подниз в целия ред. Пример:
`/v1/saft/nomenclatures?kind=invoice_types&search=фактура`.

Екранът е `/nomenclatures`. Сметкопланът е `/accounts` и зарежда едновременно
`GET /v1/accounts` и `GET /v1/saft/nomenclatures?kind=accounts`.

## Seed на сметкоплана

`POST /v1/accounts/seed-saft` и стартът на API-то викат една и съща логика.

- Четат се съществуващите `number`.
- Празен код се пропуска. Съществуващ номер се брои в `skipped` и името му остава.
- Новите редове са `company_id = 1`, `number` = кодът, `name` = името от
  каталога, `saft_account_type` = същият код, `analytic_type` = `none`.
- Записът е на партиди по 40 реда в един `INSERT`.
- Отговорът е `{"created","skipped","total"}`. Повторен seed върху пълен
  план е `created: 0`, `skipped: 360`, `total: 360`.

Миграция `3` (`saft_accounts`) пуска това веднъж при старт. Бутонът в
екрана може да го пусне пак: дублирани номера не се създават.

В bagabuch сметкопланът е общ, без `company_id`. Тук таблицата има
`company_id` и seed пише `1`.

## XML експорт

`GET /v1/saft/export?period=YYYY-MM-01&mode=monthly|annual|ondemand`
чете активната фирма и връща `application/xml`. Подробностите са в
[api.md](api.md).

Официалната схема е BG_SAFT_Schema V1.0.1. Месечният файл следва нейния ред:
сметкоплан, клиенти, доставчици, данъчна таблица, мерни единици, продукти,
дневник и после фактури с плащания между тях. Годишният файл е за ДМА.
Файлът при поискване е за склад.

Честни граници, които XSD не приема при празни данни:

- Месец без осчетоводено банково движение има `Payments` с нула записа. `Payment` е задължителен.
- Година без придобит актив няма `AssetTransaction`.
- Склад без стокова наличност няма `PhysicalStockEntry`. Няма и дневник на движения, затова няма `StockMovement`.
- `DefaultCurrencyCode` в схемата е само `EUR`.
- Чуждите полета в `Ownership` са задължителни. Българска майка се повтаря там с държава `BG`.
- Стоковият код на продукта е `0`. Амортизацията за периода е `0.00`. Доставчикът на ДМА е „Неизвестен“.
- Съпоставяне на движения (`CorrespondingAccountsReport`) не се изнася.

Демо октомври 2026 на Бага ООД минава по суми: дневник 1500.00 / 1500.00,
продажба `0000000001` 1000.00 / 1200.00, покупка `0000000042` 250.00 / 300.00.
`xmllint` отказва този месец само заради липсващия `Payment`.
