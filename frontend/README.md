# ytbuch — интерфейс

Next.js на порт **3020**. Продуктовата документация е в
[`../README.md`](../README.md) и [`../docs/README.md`](../docs/README.md).

```bash
npm install
npm run dev
```

Отвори http://127.0.0.1:3020. Вход: `superadmin` / `123+123`.

Браузърът говори само с този порт. Next препраща `/v1`, `/health` и `/ready`
към API-то на `127.0.0.1:8092` (`YTBUCH_API_PROXY`, ако е зададена).
Порт 3000 не се ползва от този интерфейс.
