import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import {
  useCrawlerActions,
  useCrawlerOverview,
} from "../hooks/useCrawlerAdmin";

export default function CrawlerDashboard() {
  const overview = useCrawlerOverview();
  const { enqueue } = useCrawlerActions();
  const [source, setSource] = useState("");
  const [series, setSeries] = useState("");
  const [priority, setPriority] = useState(5);
  const data = overview.error ? undefined : overview.data;
  return (
    <Stack spacing={3}>
      <Stack direction="row" spacing={2}>
        <Typography variant="h5" component="h2">
          Crawler queue
        </Typography>
        <Button onClick={() => overview.refetch()}>Refresh</Button>
      </Stack>
      <Typography>
        Queue state is polled every 15 seconds. Active workers are workers
        holding jobs; recent activity does not prove process health.
      </Typography>
      {overview.isLoading && (
        <CircularProgress aria-label="Loading crawler data" />
      )}
      {overview.error && (
        <Alert severity="error">{overview.error.message}</Alert>
      )}
      {data && (
        <>
          <Typography>
            Workers holding jobs: {data.crawlerStatus?.activeWorkers ?? 0}. Last
            successful job: {data.crawlerStatus?.lastCrawl || "None recorded"}.
          </Typography>
          <Typography>
            Next waiting job due:{" "}
            {data.crawlerStatus?.nextScheduledCrawl || "No waiting jobs"}.
          </Typography>
          <Box sx={{ overflowX: "auto" }}>
            <Table aria-label="Queue counts">
              <TableHead>
                <TableRow>
                  {[
                    "Pending",
                    "Processing",
                    "Retrying",
                    "Completed",
                    "Failed",
                  ].map((label) => (
                    <TableCell key={label}>{label}</TableCell>
                  ))}
                </TableRow>
              </TableHead>
              <TableBody>
                <TableRow>
                  {[
                    data.queueStatistics?.pendingItems,
                    data.queueStatistics?.processingItems,
                    data.queueStatistics?.retryingItems,
                    data.queueStatistics?.completedItems,
                    data.queueStatistics?.failedItems,
                  ].map((count, index) => (
                    <TableCell key={index}>{count ?? 0}</TableCell>
                  ))}
                </TableRow>
              </TableBody>
            </Table>
          </Box>
          <Typography variant="h6" component="h3">
            Enqueue crawl
          </Typography>
          <Typography>
            Requires admin.crawlers:manage. Leave series IDs blank to enqueue
            catalog discovery. These are queue requests; the worker runs them
            asynchronously.
          </Typography>
          <TextField
            select
            label="Source"
            value={source}
            onChange={(event) => setSource(event.target.value)}
          >
            {(data.crawlerSources || []).map((item) => (
              <MenuItem key={item.source} value={item.source}>
                {item.name}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            label="Series external IDs"
            helperText="Optional; separate IDs with commas or newlines"
            multiline
            value={series}
            onChange={(event) => setSeries(event.target.value)}
          />
          <TextField
            label="Priority"
            type="number"
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
            slotProps={{ htmlInput: { min: 1, max: 10 } }}
          />
          <Button
            variant="contained"
            disabled={
              !source ||
              enqueue.isPending ||
              !Number.isInteger(priority) ||
              priority < 1 ||
              priority > 10
            }
            onClick={() =>
              enqueue.mutate({
                source,
                seriesIds: series
                  .split(/[\n,]/)
                  .map((id) => id.trim())
                  .filter(Boolean),
                priority,
              })
            }
          >
            Enqueue crawl
          </Button>
          {enqueue.error && (
            <Alert severity="error">{enqueue.error.message}</Alert>
          )}
          {enqueue.data && (
            <Alert severity="success">
              {enqueue.data.triggerCrawl.enqueuedCount} new jobs enqueued.
              Active duplicates were skipped.
            </Alert>
          )}
        </>
      )}
    </Stack>
  );
}
