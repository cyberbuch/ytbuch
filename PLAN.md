# ytbuch — миграция на bagabuch

Същата счетоводна програма като bagabuch. Друг стек:

| Слой | bagabuch | ytbuch |
|------|----------|--------|
| API | Baga + fmrbaga :8080 | Rust + Axum :8092 |
| База | boilaDB :6575 | YottaDB r2.06 + Octo :1337 |
| UI | Next.js :3000 | същият Next.js :3020 |

3000 и 3010 остават заети от bagabuch и 7x7office. YottaDB GUI е на :9080.

Правилата за счетоводството не се сменят: черно сторно (отрицателен знак,
без обръщане на Дт/Кт), парите са низове в JSON, `company_id` дели данните.

## Какво вече върви

Текущото състояние е в [`docs/`](docs/README.md). Накратко:

- Контейнер `ytbuch-yottadb` (`download.yottadb.com/yottadb/yottadb-debian`).
  Octo: `127.0.0.1:1337`, потребител `ydb`, парола `ydbrocks`.
  Данните са в volume `ytbuch_ydb-data`. GUI: `:9080`.
- API `:8092` и UI `:3020`. Вход `superadmin` / `123+123`.
- Фирми, активна фирма, табло, четене на настройките на фирмата.
- Демо фирма **Бага ООД** с фактури, дневник, контрагенти и стока.
- Номенклатури на НАП и сметкоплан: четене и seed. Липсващите кодове се
  допълват при старт. Повторен seed не дублира номер.

## Граници на Octo (проверени на този контейнер)

- `CREATE TABLE`, `INSERT`, `SELECT`, `UPDATE`, `DELETE` минават.
- `BEGIN` минава, но `INSERT`/`UPDATE` вътре в транзакция връщат
  `Updates while in a transaction are not yet implemented`.
- Няма `CREATE INDEX`, `PREPARE`, `TIMESTAMPTZ`, `SERIAL`.
- Разширеният Postgres протокол (Parse/Bind) забива при `INSERT`
  (сървърът връща `NoData`, а tokio-postgres не праща Bind).
  API-то ползва само simple query.
- Идентификаторите са `INTEGER`, следващият е `MAX(id)+1`.
- Часовете и сумите са `VARCHAR`. В JSON сумите остават низ.

Запис на фактура + редове + дневник не може да е една SQL транзакция.
Докато Octo не го даде, такъв пост ще мине през Rust драйвера на YottaDB
(`yottadb`), не през Octo SQL. Обикновеният CRUD остава на SQL.

## Фази

1. **Вход и фирми** — готово. `/health`, `/ready`, `/v1/auth/token`, `/v1/me`,
   фирми, активна фирма, табло от фактурите. UI: http://127.0.0.1:3020
   API: http://127.0.0.1:8092.
2. **Сметкоплан, контрагенти, стоки** — четенето и seed на сметкоплана от НАП
   са готови, както и списъците `/v1/counterparts` и `/v1/products`.
   Създаване и редакция на отделен ред още не.
3. **Фактури и дневник** — демо редовете се четат (`/v1/invoices`, `/v1/journal`).
   Чернова, пост и черно сторно предстоят. Атомарността е през YottaDB
   драйвера, ако Octo още няма транзакции.
4. **ДДС, периоди, начални салда.**
5. **Банки, ДМА, склад, производство, дивиденти.**
6. **SAF-T XML, справки, печат, UBL.** Номенклатурите и seed на плана са във фаза 2.
7. **Регистрация, парола, 2FA, настройки.**

API-то пази пътищата и JSON формата на bagabuch (`items`/`count`,
`{"detail":...}`), за да работи копираният Next.js без нов клиент.

## Пускане

```
docker compose -f app-product/ytbuch/deploy/docker-compose.yml up -d
cd app-product/ytbuch/backend && cargo run
cd app-product/ytbuch/frontend && npm install && npm run dev
```

Вход както в bagabuch, докато няма друг потребител: `superadmin` / `123+123`.
