import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AccountPicker } from "./AccountPicker";

const accounts = [
  { number: "601", name: "Разходи за възнаграждения" },
  { number: "602", name: "Разходи за социални осигуровки" },
  { number: "611", name: "Разходи за външни услуги" },
];

describe("AccountPicker", () => {
  it("shows all accounts without a search", () => {
    render(<AccountPicker value="" accounts={accounts} onChange={() => {}} />);
    expect(screen.getAllByRole("option")).toHaveLength(3);
  });

  it("filters by number or name", () => {
    render(<AccountPicker value="външни" accounts={accounts} onChange={() => {}} />);
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(1);
    expect(options[0]).toHaveTextContent("611 — Разходи за външни услуги");
  });

  it("picking from the list reports the account number", () => {
    const onChange = vi.fn();
    render(<AccountPicker value="60" accounts={accounts} onChange={onChange} />);
    fireEvent.change(screen.getByRole("listbox"), { target: { value: "602" } });
    expect(onChange).toHaveBeenCalledWith("602");
  });

  it("keeps free text so prefixes like 60201 can be entered", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <AccountPicker value="" accounts={accounts} onChange={onChange} />
    );
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "60201" } });
    expect(onChange).toHaveBeenCalledWith("60201");
    // при стойност извън списъка търсачката не предлага съвпадения
    rerender(<AccountPicker value="60201" accounts={accounts} onChange={onChange} />);
    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });
});
