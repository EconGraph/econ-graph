import {
  Alert,
  Button,
  CircularProgress,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import {
  useCrawlerActions,
  useCrawlerOverview,
} from "../hooks/useCrawlerAdmin";

export default function CrawlerConfig() {
  const overview = useCrawlerOverview();
  const { schedule } = useCrawlerActions();
  return (
    <Stack spacing={2}>
      <Typography variant="h5" component="h2">
        Source scheduling
      </Typography>
      <Typography>
        Adapters come from this server build. Enabled controls future automatic
        scheduling; disabling a source leaves existing and in-flight jobs alone.
        Explicit enqueue requests remain available to operators.
      </Typography>
      <Typography>
        Worker count, timeouts and rate-limit policies are deployment
        configuration. This page does not change worker processes or create
        unsupported source adapters.
      </Typography>
      {overview.isLoading && (
        <CircularProgress aria-label="Loading source scheduling" />
      )}
      {overview.error && (
        <Alert severity="error">{overview.error.message}</Alert>
      )}
      {schedule.error && (
        <Alert severity="error">{schedule.error.message}</Alert>
      )}
      {schedule.isSuccess && (
        <Alert severity="success">Scheduling configuration saved.</Alert>
      )}
      <Table aria-label="Source scheduling">
        <TableHead>
          <TableRow>
            <TableCell>Source</TableCell>
            <TableCell>Stored scheduling</TableCell>
            <TableCell>Interval (hours)</TableCell>
            <TableCell>Action</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {((!overview.error && overview.data?.crawlerSources) || []).map(
            (item) => (
              <TableRow key={item.source}>
                <TableCell>
                  {item.name} ({item.source})
                </TableCell>
                <TableCell>
                  {item.enabled === null
                    ? "Not initialized"
                    : item.enabled
                      ? "Enabled"
                      : "Disabled"}
                </TableCell>
                <TableCell>
                  {item.crawlFrequencyHours ?? "Not initialized"}
                </TableCell>
                <TableCell>
                  <Button
                    disabled={item.enabled === null || schedule.isPending}
                    onClick={() =>
                      schedule.mutate({
                        source: item.source,
                        enabled: !item.enabled,
                      })
                    }
                  >
                    {item.enabled ? "Disable" : "Enable"} scheduling for{" "}
                    {item.source}
                  </Button>
                </TableCell>
              </TableRow>
            ),
          )}
        </TableBody>
      </Table>
      <Typography>
        Changes require admin.crawlers:manage and are audited. Read access
        requires admin.system:read. A missing stored row is initialized by
        crawler startup, never fabricated by this UI.
      </Typography>
    </Stack>
  );
}
