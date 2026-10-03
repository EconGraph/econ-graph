import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import { vi } from "vitest";
import SecCrawlerManager from "../SecCrawlerManager";
import { useSecCrawler } from "../../../hooks/useSecCrawler";
import type { Company, SecCrawlResult } from "../../../types";

vi.mock("../../../hooks/useSecCrawler");
vi.mock("../CompanySearch", () => ({
  default: ({
    onCompanySelect,
  }: {
    onCompanySelect: (company: Company) => void;
  }) => <button onClick={() => onCompanySelect(company)}>Select Apple</button>,
}));

const company: Company = {
  id: "1",
  cik: "0000320193",
  name: "Apple Inc.",
  ticker: "AAPL",
  is_active: true,
  created_at: "2023-01-01T00:00:00Z",
  updated_at: "2023-01-01T00:00:00Z",
};
const result: SecCrawlResult = {
  operation_id: "operation-1",
  cik: company.cik,
  filings_downloaded: 15,
  filings_processed: 0,
  errors: 0,
  start_time: "2023-01-01T00:00:00Z",
  status: "completed",
};
const crawlCompany = vi.fn();
const importRssFeed = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  crawlCompany.mockResolvedValue(result);
  vi.mocked(useSecCrawler).mockReturnValue({
    crawlCompany,
    importRssFeed,
    isCrawling: false,
    progress: 0,
    status: "idle",
  });
});

async function openCrawlConfirmation() {
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  fireEvent.click(await screen.findByRole("button", { name: "Start Crawl" }));
  return screen.findByRole("dialog");
}

describe("SecCrawlerManager", () => {
  it("requires company selection before allowing configuration", async () => {
    render(<SecCrawlerManager />);
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Select Apple" }));
    expect(await screen.findByLabelText("Form Types")).toHaveValue("10-K,10-Q");
  });

  it("honors a supplied company and confirms before starting the crawl", async () => {
    const onComplete = vi.fn();
    render(
      <SecCrawlerManager company={company} onCrawlComplete={onComplete} />,
    );
    const dialog = await openCrawlConfirmation();
    expect(within(dialog).getByText("Apple Inc.")).toBeInTheDocument();
    expect(crawlCompany).not.toHaveBeenCalled();
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Start Crawl" }),
    );
    await waitFor(() => expect(onComplete).toHaveBeenCalledWith(result));
    expect(crawlCompany).toHaveBeenCalledWith({
      cik: company.cik,
      form_types: "10-K,10-Q",
      start_date: undefined,
      end_date: undefined,
      exclude_amended: false,
      exclude_restated: false,
      max_file_size: 52428800,
    });
    expect(screen.getByText(/Filings Downloaded: 15/)).toBeInTheDocument();
  });

  it("sends edited filters to the crawl service", async () => {
    render(<SecCrawlerManager company={company} />);
    fireEvent.change(screen.getByLabelText("Form Types"), {
      target: { value: "8-K" },
    });
    fireEvent.change(screen.getByLabelText("Start Date"), {
      target: { value: "2023-01-01" },
    });
    fireEvent.change(screen.getByLabelText("End Date"), {
      target: { value: "2023-12-31" },
    });
    fireEvent.change(screen.getByLabelText("Max File Size (bytes)"), {
      target: { value: "1024" },
    });
    fireEvent.click(screen.getByLabelText("Exclude Amended Filings"));
    fireEvent.click(screen.getByLabelText("Exclude Restated Filings"));
    const dialog = await openCrawlConfirmation();
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Start Crawl" }),
    );
    await waitFor(() =>
      expect(crawlCompany).toHaveBeenCalledWith({
        cik: company.cik,
        form_types: "8-K",
        start_date: "2023-01-01",
        end_date: "2023-12-31",
        exclude_amended: true,
        exclude_restated: true,
        max_file_size: 1024,
      }),
    );
  });

  it("cancels without issuing a mutation", async () => {
    render(<SecCrawlerManager company={company} />);
    const dialog = await openCrawlConfirmation();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(crawlCompany).not.toHaveBeenCalled();
  });

  it("rejects an inverted date range before contacting the service", async () => {
    const onError = vi.fn();
    render(<SecCrawlerManager company={company} onCrawlError={onError} />);
    fireEvent.change(screen.getByLabelText("Start Date"), {
      target: { value: "2024-01-01" },
    });
    fireEvent.change(screen.getByLabelText("End Date"), {
      target: { value: "2023-01-01" },
    });
    const dialog = await openCrawlConfirmation();
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Start Crawl" }),
    );
    expect(
      await screen.findByText("Start Date must not be after End Date"),
    ).toBeInTheDocument();
    expect(crawlCompany).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(expect.any(Error));
  });

  it("reports service failures and permits a retry", async () => {
    const failure = new Error("Service unavailable");
    crawlCompany.mockRejectedValueOnce(failure);
    const onError = vi.fn();
    render(<SecCrawlerManager company={company} onCrawlError={onError} />);
    const dialog = await openCrawlConfirmation();
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Start Crawl" }),
    );
    expect(await screen.findByText("Service unavailable")).toBeInTheDocument();
    expect(onError).toHaveBeenCalledWith(failure);
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Start Crawl" }),
    );
    expect(
      await screen.findByText(/Filings Downloaded: 15/),
    ).toBeInTheDocument();
  });

  it("shows progress and announces status while crawling", () => {
    vi.mocked(useSecCrawler).mockReturnValue({
      crawlCompany,
      importRssFeed,
      isCrawling: true,
      progress: 50,
      status: "crawling",
    });
    render(<SecCrawlerManager company={company} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "50",
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Status: crawling (50%)",
    );
    expect(
      screen.getByRole("button", { name: "Import RSS Feed" }),
    ).toBeDisabled();
  });
});
