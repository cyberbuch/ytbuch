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
| GET | `/v1/auth/registration-status` | `{"ok":1,"registration_enabled":0\|1}`. Липсващ ключ е `1` |
| POST | `/v1/auth/register` | име, email, парола, фирма, ЕИК. `201` с `company_id`. Роля `admin` |
| POST | `/v1/auth/forgot-password` | `{"email"}`. Винаги едно и също `message`, ако акаунтът съществува или не |
| POST | `/v1/auth/reset-password` | `{"token","password","password_confirmation"}`. Токенът важи 1 час и веднъж |
| GET | `/v1/me` | `sub`, `username`, `id`, `email`, `name`, `is_super_admin`, `role_id`, `totp_enabled`, `permissions` |
| POST | `/v1/auth/mfa` | `{"mfa_token","code"}` → същият token отговор. Кодът е TOTP или резервен |

Празно `email`/`sub` е `422` `email или sub е задължителен`.
Неактивен потребител е `403` `потребителят е неактивен`.
Потребителят за вход не се сменя. Имейлът е отделно поле. И двете влизат.

Регистрацията е включена, докато `auth.registration_enabled` не е `0` или `false`.
Спряна регистрация е `403` `Регистрацията е спряна.`
Имейл без `@` е `422` `валиден email е задължителен`.
Празно име е `422` `името е задължително`.
Парола под 8 символа е `422` `паролата трябва да е поне 8 символа`.
Различни пароли са `422` `паролите не съвпадат`.
Празно име на фирма е `422` `име на фирмата е задължително`.
Празен ЕИК е `422` `ЕИК е задължителен`.
Зает имейл е `422` `потребител с този email вече съществува`.
Зает ЕИК е `409` `фирма с този ЕИК вече съществува`.
Успехът е `201` `Регистрацията е успешна. Можете да влезете.` и `company_id`.
Името за вход започва равно на имейла. Активната фирма е новата.
Новата фирма не получава собствен сметкоплан и няма серия документи.

Празен email при забравена парола е `422` `Email е задължителен`.
Иначе отговорът е `200` `Ако съществува акаунт с този email, ще получите линк за възстановяване.`
Токенът се пази като SHA-256 в `password_reset_tokens` за 3600 секунди, дори без писмо.
Писмото тръгва само ако `app.url` не е празен, `smtp.enabled` не е `0`/`false` и `smtp.host` е зададен.
Темата е `Възстановяване на парола — ytbuch`. Линкът е `{app.url}/reset-password?token=...`.
Липсващ токен при смяна е `422` `Невалиден или липсващ токен`.
Непознат, използван или изтекъл токен е `422` `Невалиден или изтекъл токен. Заявете нов линк.`
Успехът е `200` `Паролата е променена. Можете да влезете.`

## Профил

Иска токен. `sub` в JWT е потребителското име.

| Метод | Път | Бележка |
|-------|-----|---------|
| PATCH | `/v1/me` | `{"name"}`. Празно име е `422` |
| POST | `/v1/me/password` | `current_password`, `new_password`. Под 8 символа е `422`. Грешна текуща е `401` |
| POST | `/v1/me/email` | `email`, `current_password`. Зает имейл е `409`. Хешът се смята с новия имейл |
| POST | `/v1/me/totp/start` | `secret`, `uri`, `qr_svg`, `recovery_codes`. Вече включена 2FA е `409` |
| POST | `/v1/me/totp/confirm` | `{"code"}`. Включва 2FA |
| POST | `/v1/me/totp/disable` | `password` и TOTP или резервен код. Триe кодовете |

QR е SVG картинка. Резервните кодове се показват само при старта. Файлът
`ytbuch-recovery-codes.txt` се сваля от браузъра. Всеки код е за едно ползване.

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
`document_series`.

| Метод | Път | Бележка |
|-------|-----|---------|
| POST | `/v1/companies/{id}/locations` | 201. Задължително `name`. Тип `OFFICE`, `STORE`, `WAREHOUSE`, `BRANCH` или `OTHER` |
| PATCH | `/v1/company-locations/{id}` | частичен |
| DELETE | `/v1/company-locations/{id}` | 204 |
| POST | `/v1/companies/{id}/beneficial-owners` | 201. Задължителни `first_name_bg` и `last_name_bg` |
| PATCH | `/v1/beneficial-owners/{id}` | частичен |
| DELETE | `/v1/beneficial-owners/{id}` | 204 |
| POST | `/v1/companies/{id}/ultimate-parents` | 201. Задължително `name_bg` |
| PATCH | `/v1/ultimate-parents/{id}` | частичен |
| DELETE | `/v1/ultimate-parents/{id}` | 204 |
| POST | `/v1/companies/{id}/document-series` | 201. `document_types` е низ или списък. `next_number` се смята от изходящите фактури |
| PATCH | `/v1/document-series/{id}` | частичен. Смяна на типове, ширина или начало пресмята `next_number` наново |
| DELETE | `/v1/document-series/{id}` | 204 |

Тип, който влезе в една серия, се маха от другите серии на същата фирма.

## Сметки, контрагенти, стоки

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/accounts` | `id`, `number`, `name`, `saft_account_type`, `analytic_type`, подредени по номер |
| POST | `/v1/accounts` | 201. Задължителни `number` и `name`. Повтарящ се номер в същата фирма е `409` |
| GET | `/v1/accounts/{id}` | 404 `сметката не е намерена` |
| PATCH | `/v1/accounts/{id}` | само подадените полета |
| DELETE | `/v1/accounts/{id}` | 204 |
| POST | `/v1/accounts/seed-saft` | `{"created","skipped","total"}`. Съществуващ номер се пропуска |
| GET | `/v1/counterparts` | четене, с адресните полета от `counterpart_extra` |
| POST | `/v1/counterparts` | 201. Задължително `name`. `region` е `BG-01`…`BG-28` или празно |
| GET | `/v1/counterparts/{id}` | 404 `контрагентът не е намерен` |
| PATCH | `/v1/counterparts/{id}` | само подадените полета |
| DELETE | `/v1/counterparts/{id}` | 204. Триe и допълнителния адрес |
| GET | `/v1/products` | четене |
| POST | `/v1/products` | 201. Задължително `name`. Услуга нулира склада |
| GET | `/v1/products/{id}` | 404 `стоката не е намерена` |
| PATCH | `/v1/products/{id}` | само подадените полета |
| DELETE | `/v1/products/{id}` | 204 |
| GET | `/v1/units` | мерни единици от каталога. По избор `search` |

Списъците връщат всички редове, без филтър по активна фирма.
Нов ред получава `company_id` на активната фирма, или `1`, ако няма такава.
`analytic_type` е `none`, `counterpart` или `product`.
`counterpart_type` е `customer`, `supplier` или `both`.

## Фактури и дневник

Списъкът с фактури е само за активната фирма. Дневникът още връща всички записи.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/invoices` | без редове. Има данъчна дата, месец, курс, отстъпка |
| POST | `/v1/invoices` | 201, чернова. Изходящ без номер взима следващия от серията. Входящ без номер е 422 |
| GET | `/v1/invoices/next-number` | `document_type`, `direction`. За `in` номерът е празен |
| GET | `/v1/invoices/{id}` | редове, контрагент и фирма. 404 `фактурата не е намерена` |
| PATCH | `/v1/invoices/{id}` | чернова. Осчетоводена е 409 |
| DELETE | `/v1/invoices/{id}` | 204. Ако е постната, маха и дневника |
| POST | `/v1/invoices/{id}/post` | 201, запис в дневника. Вече постната е 409. Проформата е 422. Затворен месец е 422 `периодът YYYY-MM е затворен` |
| POST | `/v1/invoices/bulk/post` | `{ids}` → `posted`, `failed`, `results` |
| POST | `/v1/invoices/bulk/unpost` | `unposted`, `failed`, `results` |
| POST | `/v1/invoices/bulk/delete` | `deleted`, `failed`, `results` |
| GET | `/v1/journal` | записи |
| GET | `/v1/journal/{id}` | плюс `lines` (`direction` е `debit` или `credit`). 404 `записът не е намерен` |

Продажба: Дт `411`, Кт приход (`702` или сметката на стоката), Кт `4532`.
Покупка: Дт `304` за стока или `602` за услуга (ако няма `602`, се ползва `60212`), Дт `4531`, Кт `401`.
Кредитно `03`/`13`/`23`: същите страни, сумите с минус. Вече отрицателна сума не се обръща втори път.
Складът се движи при пост на тип `01` и `11`.
Проверката за затворен месец е преди записа в дневника и ползва `accounting_month`.

## Печат и UBL

Печатът чете фактурата и не пише в дневника. Грешен `format` е 422 `format трябва да е pdf/docx/odt`.
Липсваща фактура е 404 `фактурата не е намерена`.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/invoices/{id}/print?format=pdf` | `pdf`, `docx` или `odt`. Празен format е pdf. `copy=1` е копие. Файлът е `{номер}_{вид}.{format}`, например `0000000001_invoice.pdf` |
| GET | `/v1/invoices/{id}/ubl` | UBL 2.1. Файлът е `ubl-invoice-{номер}.xml` или `ubl-credit-note-{номер}.xml` |
| GET | `/v1/internal-docs/{id}/print?format=pdf` | същите три формата. Файлът е `protokol-{номер}.{format}` |

Входящата фактура на печата слага фирмата като получател. В UBL фирмата остава доставчик, а контрагентът клиент.
Кредитното известие е `CreditNote` с код 381. Проформата е фактура с код 325. Валута различна от EUR и курс различен от 1 добавят `TaxCurrencyCode` EUR.

## SAF-T XML

Чете активната фирма и не пише в дневника. Схемата е BG_SAFT_Schema V1.0.1.
Празен или грешен `period` е 400 `period е задължителен (YYYY-MM-01)`.
Без активна фирма е 422 `няма активна фирма за експорт`. Липсващ ред на фирмата е 422 `няма фирма за експорт`.
Непознат `mode` става `monthly`. Файлът е `saft_{period}_{mode}.xml`, `application/xml; charset=utf-8`.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/saft/export?period=YYYY-MM-01&mode=monthly` | месец: сметкоплан, страни, данъци, продукти, дневник, фактури, плащания |
| GET | `/v1/saft/export?period=YYYY-MM-01&mode=annual` | годината на периода: ДМА и придобивания. Периодът в заглавката е 1–12 |
| GET | `/v1/saft/export?period=YYYY-MM-01&mode=ondemand` | видове движения, мерни единици, продукти, наличност, движения на стоки |

Заглавката е версия `007`, област от каталога (`София` е `BG-22`), контакт на МОЛ, IBAN и `Ownership`.
Празно `is_part_of_group` става `1`. Ако майката е само българска, чуждите полета повтарят нейното име и държава `BG`.
Празни софтуерни полета стават `ytbuch` / `YTBUCH` / `1.0`.
`DefaultCurrencyCode` в схемата е само `EUR`. Експортът пише валутата на фирмата.

Месецът филтрира фактури и дневник по `YYYY-MM`. Проформата липсва. Черновата влиза.
Продажбите са кредит, покупките дебит. Сметката на продажба е `411` / `702`, на покупка `401` / `601`, освен ако фактурата сочи към запис в дневника.
ДДС 20 / 9 / 0 е `100211` / `100213` / `100214`. Основание при нулев ДДС е `100219`.
Ред в дневника с ДДС е `100211`, защото ставката не е на реда. Ред без ДДС е `000` / `000000`.
Клиентът е на сметка с префикс `411`, доставчикът на `401`. Идентификаторът е ЕИК, иначе `0`.
Продуктът в номенклатурата е с `Description` и стоков код `0`. Мярната единица има описание.
Плащанията са само осчетоводени банкови движения. Без такива `Payments` е с нула записа и XSD отказва, защото `Payment` е задължителен.

Годишният файл няма дневник и фактури. Активът е по сметка `204`, номер `FA-<id>` ако инвентарният е празен.
Доставчикът е „Неизвестен“. Амортизацията за периода е `0.00`, защото се пази само натрупаната.
Година без придобиване няма `AssetTransaction` и XSD отказва.

При поискване наличността е текущата, като начало и край. Няма дневник на складови движения, затова количествата са `0`.
Празен склад няма `PhysicalStockEntry`, а `StockMovement` липсва. И двете са задължителни в XSD.

Подаването за средни и малки фирми започва през 2028.

## Периоди

Няма ред за фирма: таблицата е една за всички, както в bagabuch. Липсващ месец е отворен.
Година под 2000 или празна става 2026. Отговорът е дванадесет месеца и поле `year`.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/accounting-periods?year=` | `items` с `year`, `month`, `status`, `closed_at`, `notes`. `id` има само ако редът е записан |
| POST | `/v1/accounting-periods/close` | `{year, month, notes, closed_at}`. Месец извън 1–12 е 422. Връща списъка за годината |
| POST | `/v1/accounting-periods/reopen` | `{year, month}`. Статус `open`, празни `closed_at` и `notes` |

## Начални салда

Не се пишат в дневника. Януари на `fiscal_year` трябва да е отворен, иначе 422.
Салдото е или дебит, или кредит. Страната следва номера по НСС.
Аналитична сметка към контрагент или продукт иска съответния идентификатор.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/opening-balances?year=` | `items`, `count`, `fiscal_year`, `total_debit`, `total_credit`, `difference`. Сумите са низове |
| POST | `/v1/opening-balances` | 201 `{status, natural_side, account_kind}`. И двете нули или и двете суми е 422 |
| PATCH | `/v1/opening-balances/{id}` | частичен запис. 404 `Opening balance not found` |
| DELETE | `/v1/opening-balances/{id}` | 204 |

## ДДС

Дневникът е за активната фирма. Влизат записите с `vat_type` `sales` или `purchase` за периода.
Ако записът сочи фактура, ставките идват от нейните редове. Иначе 20%, или без ДДС при нулев данък.
Демо покупката в дневника се вижда и докато фактурата е чернова, защото източникът е дневникът.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/vat/registers?period=` | `period` е `YYYY-MM` или `YYYYMM`. Празен е 400. Пакетът има `purchases`, `sales`, `cells`, `due`, `refund` |
| GET | `/v1/vat/return` | същият отговор |
| GET | `/v1/vat/export?period=&type=` | `deklar`, `pokupki`, `prodagbi` са Windows-1251. `zip` е `PPDDS_YYYYMM.zip` без компресия |
| GET | `/v1/vat-exemptions?search=` | каталог VATEX. `items` и `count` |

Клетките са `01-01` … `01-82` като низ. `due` е клетка 01-50, `refund` е 01-60.

## Банки, ДМА, склад, производство, дивиденти

Сумите в отговора са низове. Затворен месец е 422 `периодът YYYY-MM е затворен`.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET, POST | `/v1/bank-accounts` | създаването е 201. Повторен IBAN в същата фирма е 409 |
| PATCH, DELETE | `/v1/bank-accounts/{id}` | триенето е 204 |
| GET | `/v1/bank-transactions` | филтър `bank_account_id` и `status` (`new`, `booked`, `allocated`) |
| POST | `/v1/bank-transactions/preview` | `{bank_account_id, file_content}` → формат, брой, дубликати |
| POST | `/v1/bank-transactions/import` | `{imported, journal_count}`. CSV и MT940. Непознат файл е 422 |
| POST | `/v1/bank-transactions/{id}/book` | ръчно осчетоводяване |
| POST | `/v1/bank-transactions/{id}/reallocate` | `{account_id}` сменя буферния ред |
| DELETE | `/v1/bank-transactions/{id}` | 204, трие и дневника |
| GET, POST | `/v1/fixed-asset-categories` | категориите са общи |
| PATCH, DELETE | `/v1/fixed-asset-categories/{id}` | |
| GET, POST | `/v1/fixed-assets` | активът е на активната фирма |
| PATCH, DELETE | `/v1/fixed-assets/{id}` | триенето маха и събитията, 204 |
| POST | `/v1/fixed-assets/depreciation/preview` | `{items, total_amount}`. Активен и неконсервиран, включително нулева вноска |
| POST | `/v1/fixed-assets/depreciation/post` | 201, един запис Дт 603 / Кт 241. Липсваща сметка е 422 |
| POST | `/v1/fixed-assets/{id}/revalue` | `{event_date, reason, new_value}` |
| POST | `/v1/fixed-assets/{id}/move` | `{event_date, reason, location_id}` |
| POST | `/v1/fixed-assets/{id}/conserve` | и `/end-conserve` |
| POST | `/v1/fixed-assets/{id}/dispose` | `write_off` → `disposed`, `sale` → `sold` |
| GET | `/v1/fixed-assets/{id}/events` | `items` |
| GET | `/v1/inventory/at-location?location_id=` | складови артикули с остатък в обекта |
| GET | `/v1/internal-docs/next-number` | `{number}` с 10 знака |
| GET, POST | `/v1/internal-docs` | поне един ред `product` или `fixed_asset` |
| GET, PATCH, DELETE | `/v1/internal-docs/{id}` | потвърден протокол не се трие, 409 |
| POST | `/v1/internal-docs/{id}/confirm` | мести артикули и ДМА. Повторен е 409 |
| GET | `/v1/internal-docs/{id}/print?format=pdf` | PDF, DOCX или ODT. Грешен format е 422 |
| GET, POST | `/v1/recipes` | име и изходен артикул са задължителни |
| GET, PATCH, DELETE | `/v1/recipes/{id}` | PATCH с `lines` подменя редовете |
| GET, POST | `/v1/production-orders` | номер `ПР-{id}`, редовете са скалирани от рецептата |
| GET, DELETE | `/v1/production-orders/{id}` | завършена поръчка не се трие, 422 |
| POST | `/v1/production-orders/{id}/confirm` | изписва материали, заприходява продукция, запис по 611 (или 601). Повторен е 409 |
| GET, POST | `/v1/dividend-distributions` | `{items, count, owners, currency}`. Данък по подразбиране 5%. Създаването е 201 и връща `dividends` |
| GET, PATCH, DELETE | `/v1/dividend-distributions/{id}` | трие се само чернова, 204 |
| POST | `/v1/dividends/{id}/pay` | `{is_paid, payment_date}`. Чернова на разпределението е 422. Няма запис в дневника |

Фактура тип `01` или `11` движи склада при поста. Продажбата добавя себестойност Дт приходна сметка (или 702) / Кт складова (или 304). Покупката заприходява партида. ДДС дневникът продължава да чете редовете на фактурата, когато `document_id` е зададен.

## Справки

Източникът е дневникът на активната фирма и началните салда за годината на `from`.
Движение преди `from`, но след 1 януари, влиза в началното салдо. Крайното е начално плюс оборот, без нетиране.
Сумите са низове с два знака. Контрагентът се филтрира по записа в дневника.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/reports` | `kind`, `from`, `to`. По избор `account` и `counterpart_id` |
| GET | `/v1/reports/export` | същите параметри и `format` `pdf`, `xlsx` или `ods` |

`kind`: `trial_balance`, `general_ledger`, `correspondence_ledger`, `chronological`, `counterpart_trial`, `counterpart_chrono`, `balance_sheet`, `income_statement`, `cash_flow`, `equity`.
Празен `kind` е оборотна. Непознат е `400` `непознат kind`.
Липсващи или обърнати дати са `400`. За справка по контрагент без `counterpart_id` е `400`.
Грешен `format` е `400` `format трябва да е pdf, xlsx или ods`.

Оборотната връща `rows` и `totals`. Главната книга връща `accounts` с `transactions`.
Кореспонденциите връщат `debit_lead` и `credit_lead`. Хронологията връща `rows` и `totals`.
Отчетът връща `rows` със `code`, `kind`, `name`, `current`, `prior`. `prior` е `0.00`.
Празни формули дават нула по редовете. Сбор без формула събира преките редове. Стандартните формули се зареждат от настройките.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/fs/lines` | без `statement`: четирите бланки. Иначе редовете на бланката |
| GET | `/v1/fs/formulas?statement=` | формулите на активната фирма |
| PUT | `/v1/fs/formulas` | `{statement, line_code, formula}`. Непозната статия или счупена формула е `422` |
| POST | `/v1/fs/formulas/defaults` | `{statement}`. Ако вече има формули, `422` |
| POST | `/v1/fs/formulas/clear` | `{statement}` трие формулите на бланката |
| POST | `/v1/fs/formulas/validate` | `{formula}` → `ok`, `error`, `tokens` |

## Номенклатури

`GET /v1/saft/nomenclatures`

- без `kind`: `{"kinds":[{"id","name"}, ...], "count":11}`
- с `kind` и по избор `search`: `{"kind","items","count"}`
- непознат `kind`: `400` `непозната номенклатура (kind)`

Търсенето е подниз, без значение на малки и големи букви, върху целия ред
от каталога. Полетата по вид са в [saft.md](saft.md).

## Глобални настройки

Ключовете са в `settings`. JSON ползва `key`, колоната е `setting_key`.
`PUT` пише само подадените ключове и не трие `активна_фирма:<sub>`.

| Метод | Път | Бележка |
|-------|-----|---------|
| GET | `/v1/system-settings` | `{key,value}`, сортирани по ключ |
| PUT | `/v1/system-settings` | `{"items":[{"key","value"}]}`. Низ, число, булев или `null`. Друг тип е `422` |
| POST | `/v1/system-settings/smtp-test` | `{"to"}`. Успех `{"message":"Имейлът е изпратен"}` |
| POST | `/v1/system-settings/s3-test` | `list` с `max_keys` 1. Успех `{"message":"Връзката с S3 е успешна. Bucket: …"}` |
| POST | `/v1/system-settings/s3-backup` | качва gzip. `message`, `filename`, `s3_key`, `size`, `tables` |
| GET | `/v1/system-settings/s3-backups` | обекти, чието име съдържа `ytbuch_backup` |
| POST | `/v1/system-settings/s3-delete` | `{"s3_key"}`. Ключ без `ytbuch_backup` е `400` |

Празен SMTP host е `400` `SMTP host е задължителен`. Празен получател е `422`
`Получател е задължителен`. Порт `465` е implicit TLS, иначе STARTTLS, ако
`smtp.use_tls` не е `0` или `false`. Подразбиране: порт `587`, име `ytbuch`.

Празен bucket е `422` `S3 bucket е задължителен`. Празен ключ е `400`
`Невалидни credentials`. Регионът по подразбиране е `us-east-1`, префиксът
`backups/`. Клиентът е path-style и приема свой endpoint. Checksum се смята
само когато магазинът го иска, за да минават MinIO и Backblaze B2.

Бекъпът чете познатите таблици и пише `INSERT`. Името е
`ytbuch_backup_ГГГГММДД_ЧЧММСС.sql.gz`. Няма SQL транзакция и няма COPY.

## Потребители, роли, курсове, VIES, сканиране

| Метод | Път | Бележка |
|-------|-----|---------|
| GET, POST | `/v1/users` | списък и създаване. Паролата е поне 8 знака |
| PATCH, DELETE | `/v1/users/{id}` | частичен PATCH. Смяна на имейл иска нова парола |
| GET, POST | `/v1/roles` | `permissions` е списък от низове. Маршрутите не го проверяват |
| PATCH, DELETE | `/v1/roles/{id}` | |
| GET | `/v1/exchange-rates` | `date`, `base`, `items`. Без дата е последният ден |
| GET | `/v1/exchange-rates/rate` | `currency`, `date`. Липсващ ден е най-близкият по-ранен |
| POST | `/v1/exchange-rates/import` | `{"mode":"daily"\|"history"}` |
| GET | `/v1/counterparts/vies-lookup` | `vat`. ЕС ДДС номер |
| POST | `/v1/counterparts/vies` | `vat_number`, `counterpart_type`. Създава контрагент |
| POST | `/v1/addresses/parse` | `address`. Mistral или локално разбиване |
| GET, POST | `/v1/saft/movement-mappings` | `kind=stock\|asset\|cash` |
| POST | `/v1/saft/movement-mappings/defaults` | отказва, ако за вида вече има редове |
| PATCH, DELETE | `/v1/saft/movement-mappings/{id}` | |
| POST | `/v1/scans/ubl` | UBL → чернова. Не записва фактура |
| POST | `/v1/scans/extract` | OCR. Без Mistral ключ е `422` |
| GET, POST | `/v1/product-name-mappings` | `counterpart_id` и `items` |
