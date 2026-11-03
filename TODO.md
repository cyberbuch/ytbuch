# TODO — ytbuch подобрения

Последна актуализация: 2026-10-09

## Готово ✅

- [x] **XSS защита** — CSP с nonce в middleware, HTML екраниране в dividends, security headers
  - `frontend/middleware.ts` — nonce + CSP headers
  - `frontend/lib/html.ts` — `escapeHtml()`
  - `frontend/app/layout.tsx` — nonce за inline скрипт
  - `frontend/app/dividends/page.tsx` — екраниране на потребителски данни
  - `frontend/next.config.ts` — fallback security headers
- [x] **Error Boundaries** — `app/error.tsx` + `app/global-error.tsx` с retry и dev error details
- [x] **Реална пагинация в API** — `backend/src/pagination.rs` с `PageQuery` (limit/offset),
  `paginate()` връща `total`, `has_more`. Обновени endpoints: accounts, products, journal,
  counterparts, invoices, companies, banks, bank-transactions, users, roles, fixed-assets,
  vat-exemptions, dividends, internal-docs, mfg, recipes, settings, pnmap, asset-ops
- [x] **Frontend пагинация** — `components/Pagination.tsx`, `api.get()` с query params,
  `ListResponse<T>` с `total`/`limit`/`offset`/`has_more`
- [x] **Ръчен запис в дневника** — `POST /v1/journal`, `PATCH /v1/journal/{id}`,
  `DELETE /v1/journal/{id}` в `backend/src/journal_manual.rs`. Само `document_type='manual'`
  записи могат да се редактират/изтриват. Валидация: балансиран Дт=Кт, отворен период.
- [x] **Приключвателни операции** — `backend/src/closing.rs`: карта `closing_map` в настройките
  на фирмата (префикс → целева сметка, най-дългият префикс печели; по подразбиране 60→611).
  `GET /v1/closing-operations/preview`, `POST /v1/closing-operations/run` — балансирана статия
  `closing` / `CLOSE-YYYY-MM` на последния ден на месеца, 409 при повторение, изтриване от дневника.
  UI: таб «Приключване» в настройките и бутон «Приключи» на всеки отворен месец.
- [x] **Frontend тестове (Vitest)** — Vitest + Testing Library + jsdom инсталирани.
  `vitest.config.ts`, `vitest.setup.ts`, `npm test` скрипт. Тестове за `escapeHtml()`
  и `Pagination` компонента (12 теста минават).
- [x] **Rate limiting в backend** — `tower_governor` + `governor` в Cargo.toml,
  `backend/src/rate_limit.rs` с `PeerIpKeyExtractor`. Лимит: burst 100, ~2 req/sec на IP.
  Layer е добавен в main.rs преди gate middleware.

## Критични (сигурност и стабилност)

- [ ] **SQL string concatenation** — въведи parameterized queries (въпреки ограниченията на Octo)
- [ ] **Error Boundaries** — добави `error.tsx` на ниво layout и сегменти
- [ ] **Rate limiting** — добави `tower-governor` или подобен middleware в backend
- [ ] **JWT в httpOnly cookies** — миграция от localStorage (по-сигурно, изисква промени в backend)

## Важни (UX и производителност)

- [ ] **Реална пагинация** — API-то връща фиксиран лимит 50 без offset/cursor
- [ ] **Филтри и търсене** — добави филтри по дата, статус, контрагент в списъците
- [ ] **Loading skeletons** — suspense boundaries и skeleton states
- [ ] **Разбиване на големи файлове** — `scan/page.tsx` (655 реда), `journal/page.tsx` (594)
- [ ] **Debounce на търсенето** — за search полетата
- [ ] **ARIA labels** — достъпност за бутони с икони

## Функционални пропуски (от status.md)

- [ ] **Ръчен запис в дневника** — `POST/PUT /v1/journal` липсва в API
- [ ] **XML банкови извлечения** — OBB, camt.053, Postbank не се четат
- [ ] **Bulk операции** — масово осчетоводяване, изтриване, печат
- [ ] **Audit trail** — история на промените (кой, кога, какво)
- [ ] **Импорт на данни** — освен UBL и банков CSV
- [ ] **Dashboard с графики** — статистики и визуализации
- [ ] **Нотификации в реално време** — WebSocket/SSE
- [ ] **Експорт на данни** — повече формати освен PDF/XLSX/ODS

## Качество на кода

- [ ] **Frontend тестове** — Vitest + Testing Library, после Playwright E2E
- [ ] **OpenAPI спецификация** — генерирай от Axum (utoipa) или пиши ръчно
- [ ] **Pre-commit hooks** — Husky + lint-staged + rustfmt + prettier
- [ ] **Валидация на форми** — Zod схеми (вече е в node_modules)
- [ ] **Commit message linting** — conventional commits

## Backend подобрения

- [ ] **Структурирани логове** — подобри tracing извеждането
- [ ] **Метрики** — Prometheus/Grafana интеграция
- [ ] **Health check за външни зависимости** — SMTP, S3, ЕЦБ, VIES
- [ ] **Connection pooling оптимизации** — текущ pool size е 8

## DevOps и инфраструктура

- [ ] **Автоматични бекъпи** — разписание за S3 backup (сега е ръчен)
- [ ] **Мониторинг** — health dashboard за контейнерите
- [ ] **CI подобрения** — повече тестове в pipeline-а

## Мобилна версия / PWA

- [ ] **Responsive design одит** — провери дали е mobile-friendly
- [ ] **PWA функционалност** — offline работа, service worker
- [ ] **Touch-friendly UI** — за таблети и телефони

## Документация

- [ ] **API документация** — интерактивна (Swagger UI / Redoc)
- [ ] **Потребителско ръководство** — за крайните потребители
- [ ] **Deployment guide** — за production среди

---

## Приоритет за следваща сесия

1. Пагинация в UI страниците (използване на Pagination компонента)
2. JWT в httpOnly cookies (миграция от localStorage)
3. SQL string concatenation → parameterized queries
4. XML банкови извлечения (OBB, camt.053, Postbank)
5. Bulk операции (масово осчетоводяване, изтриване, печат)

## Бележки

- Qdrant контейнер (`kilo-qdrant`) е стартиран на `127.0.0.1:6333` за Kilo Code индекс
- Docker volume за Qdrant: `kilo_qdrant_data`
- CSP nonce е в `frontend/middleware.ts`
- HTML екраниране: `frontend/lib/html.ts` → `escapeHtml()`
