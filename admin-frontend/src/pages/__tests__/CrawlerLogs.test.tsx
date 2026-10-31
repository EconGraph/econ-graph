import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import CrawlerLogs from "../CrawlerLogs";
import { useCrawlerJobs } from "../../hooks/useCrawlerAdmin";
vi.mock("../../hooks/useCrawlerAdmin", () => ({ useCrawlerJobs: vi.fn() }));

it("renders durable execution records, filters them, and can stop polling", () => {
  vi.mocked(useCrawlerJobs).mockReturnValue({
    data: {
      crawlerJobs: [
        {
          id: "job",
          source: "FRED",
          seriesId: "GDP",
          kind: "fetch_series",
          status: "failed",
          retryCount: 3,
          maxRetries: 3,
          updatedAt: "2026-01-01",
          errorMessage: "transient error",
        },
      ],
    },
    refetch: vi.fn(),
  } as any);
  render(<CrawlerLogs />);
  expect(
    screen.getByRole("table", { name: "Crawler execution history" }),
  ).toHaveTextContent("FRED: GDP");
  expect(screen.getByText("transient error")).toBeInTheDocument();
  expect(
    screen.queryByText(/CPU Usage|Memory Usage|Processing Rate/),
  ).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Source filter"), {
    target: { value: "BLS" },
  });
  expect(useCrawlerJobs).toHaveBeenLastCalledWith("BLS", "", 0, true);
  fireEvent.click(screen.getByRole("switch", { name: "Poll history" }));
  expect(useCrawlerJobs).toHaveBeenLastCalledWith("BLS", "", 0, false);
});
