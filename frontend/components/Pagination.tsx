"use client";

// Pagination — бутони за страници. Показва се само при total > limit.

interface PaginationProps {
  total: number;
  limit: number;
  offset: number;
  onPageChange: (offset: number) => void;
}

export function Pagination({ total, limit, offset, onPageChange }: PaginationProps) {
  if (total <= limit) return null;

  const currentPage = Math.floor(offset / limit) + 1;
  const totalPages = Math.ceil(total / limit);
  const hasPrev = offset > 0;
  const hasNext = offset + limit < total;

  const goTo = (page: number) => {
    const newOffset = (page - 1) * limit;
    onPageChange(Math.max(0, Math.min(newOffset, total - 1)));
  };

  const pages: number[] = [];
  const start = Math.max(1, currentPage - 2);
  const end = Math.min(totalPages, start + 4);
  for (let i = start; i <= end; i++) pages.push(i);

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4, marginTop: 16, flexWrap: "wrap" }}>
      <button
        className="btn btn-sm"
        disabled={!hasPrev}
        onClick={() => goTo(currentPage - 1)}
        aria-label="Предишна страница"
      >
        ‹
      </button>

      {start > 1 && (
        <>
          <button className="btn btn-sm" onClick={() => goTo(1)}>1</button>
          {start > 2 && <span className="muted">…</span>}
        </>
      )}

      {pages.map((p) => (
        <button
          key={p}
          className={`btn btn-sm ${p === currentPage ? "btn-primary" : ""}`}
          onClick={() => goTo(p)}
          aria-label={`Страница ${p}`}
          aria-current={p === currentPage ? "page" : undefined}
        >
          {p}
        </button>
      ))}

      {end < totalPages && (
        <>
          {end < totalPages - 1 && <span className="muted">…</span>}
          <button className="btn btn-sm" onClick={() => goTo(totalPages)}>{totalPages}</button>
        </>
      )}

      <button
        className="btn btn-sm"
        disabled={!hasNext}
        onClick={() => goTo(currentPage + 1)}
        aria-label="Следваща страница"
      >
        ›
      </button>

      <span className="muted" style={{ marginLeft: 8, fontSize: 13 }}>
        {offset + 1}–{Math.min(offset + limit, total)} от {total}
      </span>
    </div>
  );
}
