# Документация — ytbuch

Счетоводна програма на Rust и Axum, YottaDB с Octo, Next.js на
порт 3020. Базова валута EUR.

Този каталог е **текущото състояние на кода**. `PLAN.md` в корена е пътната
карта на миграцията. `start-ytbuch.md` е първоначалната бележка.

## Карта

| Файл | За какво |
|------|----------|
| [architecture.md](architecture.md) | Процеси, модули, потоци, фирма, права, Octo |
| [api.md](api.md) | HTTP пътища, които реално отговарят |
| [data.md](data.md) | Таблици, миграции, демо фирма |
| [saft.md](saft.md) | Номенклатури на НАП и seed на сметкоплана |
| [status.md](status.md) | Какво е готово и какво още е празен екран |

## Стартиране

```bash
docker compose -f deploy/docker-compose.yml up -d --build
docker compose -f deploy/docker-compose.yml ps
```

Вход: **`superadmin` / `123+123`**.
Отвори http://127.0.0.1:3020.
API: http://127.0.0.1:8092.
