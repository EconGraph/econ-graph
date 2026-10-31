import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import CrawlerConfig from "../CrawlerConfig";
import {
  useCrawlerActions,
  useCrawlerOverview,
} from "../../hooks/useCrawlerAdmin";
vi.mock("../../hooks/useCrawlerAdmin", () => ({
  useCrawlerActions: vi.fn(),
  useCrawlerOverview: vi.fn(),
}));

it("shows real scheduler configuration and sends a scoped source change", () => {
  const mutate = vi.fn();
  vi.mocked(useCrawlerActions).mockReturnValue({ schedule: { mutate } } as any);
  vi.mocked(useCrawlerOverview).mockReturnValue({
    data: {
      crawlerSources: [
        { source: "FRED", name: "FRED", enabled: true, crawlFrequencyHours: 6 },
        { source: "BLS", name: "BLS", enabled: null },
      ],
    },
  } as any);
  render(<CrawlerConfig />);
  fireEvent.click(
    screen.getByRole("button", { name: "Disable scheduling for FRED" }),
  );
  expect(mutate).toHaveBeenCalledWith({ source: "FRED", enabled: false });
  expect(
    screen.getByRole("button", { name: "Enable scheduling for BLS" }),
  ).toBeDisabled();
  expect(
    screen.getByText(/existing and in-flight jobs alone/),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("checkbox", { name: /maintenance/i }),
  ).not.toBeInTheDocument();
});
