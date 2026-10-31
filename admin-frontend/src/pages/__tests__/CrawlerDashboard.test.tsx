import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import CrawlerDashboard from "../CrawlerDashboard";
import {
  useCrawlerActions,
  useCrawlerOverview,
} from "../../hooks/useCrawlerAdmin";
vi.mock("../../hooks/useCrawlerAdmin", () => ({
  useCrawlerActions: vi.fn(),
  useCrawlerOverview: vi.fn(),
}));

describe("CrawlerDashboard executable controls", () => {
  beforeEach(() => {
    vi.mocked(useCrawlerActions).mockReturnValue({
      enqueue: { mutate: vi.fn() },
    } as any);
    vi.mocked(useCrawlerOverview).mockReturnValue({
      data: {
        crawlerStatus: { activeWorkers: 2 },
        queueStatistics: { pendingItems: 7 },
        crawlerSources: [{ source: "FRED", name: "FRED" }],
      },
      refetch: vi.fn(),
    } as any);
  });
  it("shows recorded queue state and no invented process controls", () => {
    render(<CrawlerDashboard />);
    expect(screen.getByText(/Workers holding jobs: 2/)).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: "Queue counts" }),
    ).toHaveTextContent("7");
    expect(
      screen.queryByRole("button", { name: /Stop|Restart|Pause|Backup/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Enqueue crawl" }),
    ).toBeDisabled();
  });
  it("does not show stale operator data after an authorization failure", () => {
    vi.mocked(useCrawlerOverview).mockReturnValue({
      data: { crawlerStatus: { activeWorkers: 2 } },
      error: new Error("Insufficient permissions"),
      refetch: vi.fn(),
    } as any);
    render(<CrawlerDashboard />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Insufficient permissions",
    );
    expect(screen.queryByText(/Workers holding jobs/)).not.toBeInTheDocument();
  });
  it("refreshes the server snapshot on demand", () => {
    render(<CrawlerDashboard />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(useCrawlerOverview().refetch).toHaveBeenCalledOnce();
  });
});
