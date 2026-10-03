# API

Базов адрес: `http://127.0.0.1:8092`. От браузъра същите пътища минават през
`http://127.0.0.1:3020`.

Защитените пътища искат `Authorization: Bearer <token>`.
Грешката е `{"detail":"..."}`.
Списъците са `{"items","count","limit":50,"offset":0}`.

`GET` под `/v1/`, за който няма маршрут, връща празен списък със статус 200.
Така копираните екрани не показват „Not Found“. `POST`, `PATCH`, `PUT` и
`DELETE` без маршрут са `404` с `{"detail":"Not Found"}`.

## Служебни

| Метод | Път | Отговор |
|-------|-----|---------|
| GET | `/health` | `{"status":"ok"}` |
| GET | `/ready`, `/readyz` | `{"status":"ok","database":"up"}`. При счупена връзка: 503 |

## Вход

| Метод | Път | Тяло / отговор |
|-------|-----|----------------|
| POST | `/v1/auth/token` | `{"sub"\|"email","password"}` → `access_token`, `token_type`, `sub`, `expires_in` |
| GET | `/v1/auth/registration-status` | `{"registration_enabled":0}` |
| GET | `/v1/me` | `sub`, `id`, `email`, `name`, `is_super_admin`, `role_id`, `totp_enabled`, `permissions` |

Празно `email`/`sub` е `422` `email или sub е задължителен`.
Неактивен потребител е `403` `потребителят е неактивен`.

## Фирми

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/companies` | списък |
| POST | `/v1/companies` | 201. Задължителни `name` и `eik`. В отговора има `saft_accounts_seeded: 0` |
| GET | `/v1/companies/{id}` | 404 `фирмата не е намерена` |
| PATCH | `/v1/companies/{id}` | само подадените полета. Липсващо поле не се трие. `null` на низ го изчиства |
| DELETE | `/v1/companies/{id}` | 204. Триe само реда на фирмата |
| GET | `/v1/active-company` | фирмата на ключ `активна_фирма:<sub>`, или `{}` |
| PUT | `/v1/active-company` | `{"company_id":1}` |
| GET | `/v1/dashboard` | суми от всички фактури в таблицата |

Таблото връща `company_id` (активната фирма), `year`, `invoices_draft`,
`invoices_posted`, `receivables`, `payables`, `vat_output`, `vat_input`,
`revenue_ytd`, `expense_ytd`, `bank_total`, `recent` (до 5 фактури, по-новият
`id` е пръв). Сумите са низове с два знака. `bank_total` е винаги `"0.00"`.
Изходяща фактура влиза във вземанията, входяща — в задълженията, включително
черновите. Филтър по `company_id` върху фактурите още няма.

Нова фирма по подразбиране: валута `EUR`, ДДС период `monthly`, метод на
запаси `WAC`, начало на годината месец `1`, държава `BG`.

## Настройки на фирма

`GET /v1/companies/{id}/settings`

Обект с `company`, `locations`, `beneficial_owners`, `ultimate_parents`,
`document_series`. Запис на тези табове още няма.

## Сметки, контрагенти, стоки

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/accounts` | `id`, `number`, `name`, `saft_account_type`, `analytic_type`, подредени по номер |
| POST | `/v1/accounts/seed-saft` | `{"created","skipped","total"}`. Съществуващ номер се пропуска |
| GET | `/v1/counterparts` | четене |
| GET | `/v1/products` | четене |

Списъците връщат всички редове, без филтър по активна фирма.
Създаване и редакция на сметка, контрагент и стока още няма: POST към тях
е `404`.

## Фактури и дневник

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/invoices` | без редове |
| GET | `/v1/invoices/{id}` | плюс `lines`. 404 `фактурата не е намерена` |
| GET | `/v1/journal` | записи |
| GET | `/v1/journal/{id}` | плюс `lines` (`direction` е `debit` или `credit`). 404 `записът не е намерен` |

Публикуване, сторно, нова фактура и нов запис още няма.

## Номенклатури

`GET /v1/saft/nomenclatures`

- без `kind`: `{"kinds":[{"id","name"}, ...], "count":11}`
- с `kind` и по избор `search`: `{"kind","items","count"}`
- непознат `kind`: `400` `непозната номенклатура (kind)`

Търсенето е подниз, без значение на малки и големи букви, върху целия ред
от каталога. Полетата по вид са в [saft.md](saft.md).
