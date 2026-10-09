import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Pagination } from "./Pagination";

describe("Pagination", () => {
  it("renders nothing when total <= limit", () => {
    const { container } = render(
      <Pagination total={10} limit={50} offset={0} onPageChange={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("shows page numbers and range info", () => {
    render(<Pagination total={100} limit={20} offset={0} onPageChange={() => {}} />);
    expect(screen.getByText("1–20 от 100")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Страница 1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Страница 5" })).toBeInTheDocument();
  });

  it("disables prev button on first page", () => {
    render(<Pagination total={100} limit={20} offset={0} onPageChange={() => {}} />);
    expect(screen.getByRole("button", { name: "Предишна страница" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Следваща страница" })).toBeEnabled();
  });

  it("disables next button on last page", () => {
    render(<Pagination total={100} limit={20} offset={80} onPageChange={() => {}} />);
    expect(screen.getByRole("button", { name: "Следваща страница" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Предишна страница" })).toBeEnabled();
  });

  it("calls onPageChange with correct offset when clicking next", () => {
    const onPageChange = vi.fn();
    render(<Pagination total={100} limit={20} offset={0} onPageChange={onPageChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Следваща страница" }));
    expect(onPageChange).toHaveBeenCalledWith(20);
  });

  it("calls onPageChange with correct offset when clicking a page number", () => {
    const onPageChange = vi.fn();
    render(<Pagination total={100} limit={20} offset={0} onPageChange={onPageChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Страница 3" }));
    expect(onPageChange).toHaveBeenCalledWith(40);
  });

  it("highlights current page", () => {
    render(<Pagination total={100} limit={20} offset={40} onPageChange={() => {}} />);
    const currentPage = screen.getByRole("button", { name: "Страница 3" });
    expect(currentPage).toHaveAttribute("aria-current", "page");
    expect(currentPage).toHaveClass("btn-primary");
  });
});
